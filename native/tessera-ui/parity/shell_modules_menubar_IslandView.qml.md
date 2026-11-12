# shell/modules/menubar/IslandView.qml

The dynamic island: idle (the spaces), compact (a live activity's info and
controls), expanded (hovered, pinned or an alert), the compact alert pill
over a fullscreen app. Rust: `surfaces/menubar/ui/island.slint` (`Island`),
what shows decided in `surfaces/menubar/src/island.rs` (`view`), data and
button handlers in `surfaces/menubar/src/lib.rs`, the host half
(`host/shell/src/island.rs`, Island.qml's service).

## Why the user's buttons "didn't work" (fixed this round)

1. **Lists rebuilt under the pointer.** Every refresh handed the island new
   models: the live activity's controls (`ModelRc::new` whenever the live
   key changed, i.e. every second for a timer or a recording), the alerts'
   rings / extra buttons / card buttons (each `set_row_data` carried new
   nested models), the bar's chips and items. Slint drops and remakes every
   element of a replaced model, so a press that started on a button ended
   on a new one: the click was lost (and hover highlights blinked). Now
   every list is a kept `tessera_core::rows::Rows` (controls per place:
   pill / card / each row; alerts keyed by uid with each alert's own lists
   kept by uid; chips keyed by id; items keyed by id).
2. **The compact pill opened under the pointer on the way to its buttons.**
   QML's `overControls` (65-66): hovering the pill's stop/pause doesn't open
   the island. Rust opened it 140 ms after any hover, so the compact
   controls faded out and the click landed on the open island. Ported
   (`over-controls`, `hover-wants`).
3. **Standalone had no timer / stopwatch / recording activities at all**
   (nobody turned the host's Timers and Recorder state into live
   activities). The host's Island service now derives them each turn
   (`derive_live`: LiveInfo's icon, title, time, detail, progress and
   controls), and their controls run (`nc` timer/stopwatch acts, `recorder`
   IPC, `screen.stopRecording`).
4. **All-at-once mode** (`island.activityMode: "all"`, the user's setting)
   showed only the focused activity. Now: the music card plus every other
   activity as a row (IslandActivity) with its controls (without reset),
   staggered 45 ms.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Hover opens 140 ms after entering (shape ±4 px), closes 260 ms after leaving | 61, 66-70 | island.slint `hover-wants`, Timers 140/260 ms | same |
| Hovering the compact pill's controls doesn't open it | 62-66 | `over-controls` | same (fixed) |
| Click on the compact pill opens it | 788-791 | `over.clicked` → `hover-open` | same |
| Right-click anywhere on it pins / unpins | 793-796 | `over.pointer-event` right up → `pin` → host `island.pin` (config `island.pinned`) | same |
| Pin button top-right (26×26, icon 14, push-pin / push-pin-slash), only open and h > 40, gone at once while closing | 770-785 | `IconButton` at `parent.width - 32`, `opacity: content-in` | same |
| Recent button top-left (clock-counter-clockwise) and the Recent list | 737-768 | — | missing (next batch: IslandRecent) |
| Compact: live activity's controls clickable (stop recording, pause timer, play/pause music…) | IslandLive | `trail` IconButtons → `control(kind, i, false)` → lib.rs `island_control` (music: MPRIS on a thread; others: host `island.live <kind> <i>`) | same (fixed: kept models, overControls) |
| Open: the focused activity's card with every control (music prev/play/next; timer reset/play/skip; recording discard/mute/pause/stop) | IslandFocusCard, LiveInfo.controls | `card` + `control(kind, i, true)` → host `island.live <kind> <i> all` | different: the card is simplified (no seek bar, volume, open-player, cast — IslandFocusCard is the next batch) |
| All at once: music card + other activities as rows with controls (no reset), 45 ms stagger, 280 ms fade + 8 px rise | 711-730, IslandActivity | `ActivityRow` list (`act-rows`), `control(kind, i, true)` with the reset-less index mapped back | same (fixed; was missing) |
| Wheel / two-finger swipe over the compact pill or the open card switches activity | 494-500, 670-674 | `over.scroll-event` → `cycle` (Wheel::step, tested) | same; the swap's collapse-and-reopen animation (110 ms in, 260 ms OutBack out) missing |
| Page dots under the card when several (one-at-a-time), click selects | 676-699 | dots drawn, `!all-mode` | different: dots not clickable yet |
| Alerts in the island, their buttons (action, action 2, more, card buttons), wheel up dismisses | 581-632 | `AlertView` Pills → `act(uid, …)` → host `island.act` (desktop-made alerts → `today.islandAct`); scroll → `dismiss` | same (fixed: kept models) |
| HUD row: mute on the symbol, drag/click/scroll the level, outputs button | IslandLevels | `AlertView` hud | same (outputs open the Sound menu, not inline) |
| Pointer resting on alerts holds their time | 109-116 | `holding` → `island.hold` | same |
| Esc dismisses the newest alert after a click on it | 117-145 | — | missing |
| Status line | 731-736 | `IslandStatus` | see IslandStatus.qml.md (fixed) |
| Size springs (w/h), corners min(tile, h/2), content fades in Theme.motion.hover, gone at once closing | 186-228 | `shape` animate 480 ms spring(0.22); `content-in` | different: Slint's spring curve, not Qt's SpringAnimation (measured close; tester to time) |
| Material: tonal surface (surfaceContainerHigh, Highest with an alert), shadow while floating open, hot tint | 251-280 | `if Theme.material: Rectangle` | same |
| Glass: black melting into glass, rim; hot ripples/arch | 298-431 | gradient rectangle, rim; glow only | different: no shaders (ripples, arch, album art as the island, edge fade) |
| Over a fullscreen app: pill mode (newest alert, click opens) | 36-53, 503-565 | `mode == 3`, `open-pill` | same |

## Test plan (Sonnet tester)

Arena with the user's settings (`island.activityMode: "all"`, livePieces as theirs), `--bin` the fixer's binary, both looks. Screen 1920×1080 scale 1; island centre x = 960, y ≈ 14 closed.

Setup inside the arena (`inbox`/`dev run`):
- music: `python3 sandbox/tools/fakeplayer.py &` (a 3:30 track, answers PlayPause/Next/Previous).
- timer: `ipc timer toggle` (starts the focus timer); stopwatch: `ipc timer stopwatch`.
- recording: `ipc recorder screen` (or Super+Shift+R area), stop later with the island.
- alerts with buttons: `ipc island demo download` (Open / Show in Folder), `ipc island demo connect` (Connect), `notify-send -A ok=OK -A no=Later "Title" "Body"` (card buttons).

1. **Compact controls** (music alone): with the player playing, the pill shows art, title, bars and prev/play/next (user's pieces). `shot compact`. Glide from (960,200) slowly to the play/pause button's x (find it in the shot, y = 14) — `P glide <x> 14 600`, wait 0.5 s: the island must NOT open (compact still). Click: the fake player pauses (pill icon turns to play within ~0.5 s; `dev run playerctl status` says Paused). Click again → Playing. Next → the title changes to "Blinding Lights".
2. Click the pill away from the buttons (on the title): it opens.
3. **Open card + rows**: start the timer and stopwatch too. Hover the island (`P glide 960 14 300`, wait 0.6 s), `shot open-all`: the music card (art, title, artist, prev/play/next) then a row "Focus · 24:5x left" with a progress line and pause + skip buttons, a row "Stopwatch · 0:0x" with lap(flag) + pause; then the status line with icons. Rows fade/rise in 45 ms apart.
4. Click the timer row's pause: within 1 s the row's icon becomes play and its time stops counting (two shots 2 s apart show the same time). Click skip: the title becomes "Break". Click the stopwatch row's pause: it stops; its reset is not shown in the row (reset only in the open card when one-at-a-time).
5. Click the card's play/pause, prev, next: the fake player follows each click (every click works first time; try 5 quick clicks on next: 5 track changes).
6. **Recording**: `ipc recorder screen`; compact pill shows a pulsing red dot, time counting, stop (user pieces). Hover onto stop slowly: the island stays compact; click stop: recording ends (pill goes, a "Recording saved" alert appears with its buttons).
7. **Alerts**: `ipc island demo download`: the island opens with the alert; click "Show in Folder" (outlined) and, after another demo, "Open" (filled): each runs and the alert leaves. `ipc island demo connect` → click Connect → the alert changes to "Connecting…". A notification card with buttons: click OK → the notification's action fires and the card leaves.
8. **Pin**: hover open, click the pin (top right, (960+196−19, 16) ≈ (1137,16)): it stays open after the pointer leaves (`P glide 960 600`, wait 1 s, shot). Click again: unpinned, it closes 260 ms after leaving. Right-click anywhere on the open island: pins/unpins likewise.
9. **Wheel**: with music + timer in one-at-a-time mode (set `island.activityMode: "one"`), scroll down over the compact pill (`P scroll 0 1` style) → the focus moves to the next activity (pill content changes).
10. Do 1, 3, 5, 8 in Glass too. Record 3 (`rec open 1.5`) for both shells and compare open timing (first frame that moves after the hover's 140 ms) with `tools/anim-timing.py`.

## Test results

## Fixes after testing (fixer, 7 Nov, batch 3 build)

- Alerts stuck after a click (batch 2 FAIL 1): the bar took the keyboard Exclusive on a click, which in Hyprland also kept the pointer, so the island never heard the pointer leave (held alerts, × visible). Now OnDemand while the alerts are pointed at (a click gives the keys; leaving takes them back), and every 250 ms while alerts are up a hover whose pointer is on another surface is let go.
- Corner buttons unreachable (batch 2 FAIL 3): the pin and Recent buttons count as the island for the hover (they sit outside its TouchArea), so moving onto them keeps it open.
- Island height (batch 1): see IslandStatus.qml.md.
- Recording stop (batch 1 FAIL): the island sends `screen.stopRecording` (and the pill's stop the same); `ipc recorder stop` failing too and wf-recorder surviving SIGINT in the arena point at host/desktop's recorder (not this group's code): reported to the lead.

Retest the FAIL steps above.

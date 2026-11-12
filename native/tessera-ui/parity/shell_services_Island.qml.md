# shell/services/Island.qml

The island's service: live activities and their focus, alerts (post /
update / remove / dismiss, burst, stack of 4, hold, sweep), history, the
watches that post alerts (music, charger, battery, timers, recording, Focus,
heat, CPU / memory), the HUD, IPC. Rust has two halves:
standalone host `host/shell/src/island.rs` (alerts and their actions,
history, watches, IPC, live activities pushed to the bar) and the menu bar's
own copy `surfaces/menubar/src/island.rs` (what the island shows: alerts
mirrored from either shell, the activities' focus, the wheel, the view
decision), wired in `surfaces/menubar/src/lib.rs`.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Music lingers 60 s after pausing (with a track) | 24-41 | lib.rs:1554-1563, `music()` 301-310 | same |
| Activities list and order (mirroring … transfer) | 44-71 | host island.rs:46 `LIVE_ORDER`, `derive_live` 446-530, `activity` 936; menubar island.rs:320 `with_music` | same |
| A new activity takes the focus; music doesn't take it from captions | 74-86 | menubar island.rs:279-289 | same |
| cycle(step) / scroll(dx, dy) via IslandCycle (notch 120, swipe = 1 step, 320 ms lock, 400 ms idle) | 87-104 | menubar island.rs:106-135, 296-317; lib.rs:1218-1223 (Slint px × 8 → angleDelta) | same (×8 is an approximation of a notch) |
| shows / customised / liveDefaults | 106-114 | host island.rs:49-61, 427-441; settings.rs `shows` | same |
| setShows (writes pieces and `known`) | 115-125 | — (Settings surface) | n/a here |
| pinned / togglePin | 127-131 | host island.rs:920-924 | same |
| forward() to the Rust island | 186-197 | host island.rs:176-193 | same |
| post: off / not armed (4 s) → nothing | 204-206, 455-461 | host island.rs:218 | same |
| "Only important" over full screen → history only | 207-211, 162-178 | host island.rs:222-225 | same |
| Same `key` replaces in place (keeps its history line) | 213-225 | host 230-239; menubar island.rs:189-193 | same |
| HUD / music update their row | 226-236 | host 241-247; menubar 195-199 | same |
| Same kind + title while up: in place, at least as long | 237-250 | host 249-258; menubar 200-205 | same |
| Burst: within 400 ms, delay 110·i, +600·i ms | 251-256 | host 259-262, 272; menubar 206-213 | same |
| At most 4, oldest first, sticky (ringing) stays | 258-265 | host 274-277; menubar 214-221 | same |
| note() + Notifications.islandSaid (a notification about the same thing stays quiet) | 155-161, 269-272 | — | missing |
| update(uid, patch), history line follows | 277-291 | host 284-303; menubar 226-232 | same |
| remove(uid, since) | 293-300 | host 306-317 | same |
| dismiss(since) keeps sticky and newer | 315-322 | host 321-324; menubar 243-245 | same |
| find(key) | 324-329 | host island.rs:870-874 (state `island.found`) | same |
| History: unrecorded kinds, same thing within 60 s updates (missed calls by caller), max 50, times, remembered buttons for transfer / usb / sysupdate / missedCall | 336-374 | host 329-367 | same |
| forget / clearHistory | 375-381 | host 908-918 | same |
| ago() | 383-386 | — | missing (no Recent list) |
| History kept across shell reloads (PersistentProperties) | 387-400 | — (in memory) | different: lost when the host restarts |
| hold(key, on): nothing times out; on release + time held + 1 s | 406-428 | host 378-396; menubar 248-262; lib.rs:1269-1273 | same |
| Sweep every 250 ms (not while held): expiring at expiry, removed 700 ms later | 434-454 | host 398-423; menubar 264-276 (sets `leaving`); lib.rs:1144-1149 | same |
| wants(kind) with newKinds | 463-468 | host 143-147 | same |
| New track → music alert (3.5 s, with art) | 470-479 | — | missing (standalone: no track watch) |
| Charging / Charger removed alert; removes the low-battery alert | 482-493 | host 586-604 | same; "Charger removed" colour `rgba(…)` isn't parsed by the bar (accent glyph) |
| Low battery at 20 / 10 / 5 %, until charged; resets above 25 % | 494-513 | host 608-622 | same |
| Timers phase ended → "Focus session done" / "Break's over" alert | 515-522 | — (timers.rs:90 sends a desktop notification) | missing |
| Recording started / saved (Open, Show in Folder) / discarded (Undo) / failed / notice | 524-548 | host/desktop recorder.rs:259-402 | same; colours "danger" / "orange" aren't hex → the bar tints them accent |
| Focus on / off alert | 550-557 | — | missing |
| Resources sampled while resources / temperature alerts are on | 562-566 | host 134-141 | same |
| Heat: hot after 30 s ≥ hotTemp, cool after 60 s ≤ hotTemp−5; one alert per spell, kept live; ask mode (Cool down / Not now) | 575-661 | host 627-690 | same |
| Hot → CoolDown.hot, Low Power via Power (ask mode: after the yes) | 600-628 | host 692-714 (`cooldown.hot`, `power.ask`), answer via `cooldown.answer` (676) | same (ask-mode Low Power delegated to the cooldown service) |
| CPU busy (4 readings > 90 %), memory nearly full (3) | 663-677 | host 955-966 (with `gate: resources`) | same |
| hud(kind, device): volume / mic / brightness row, 1.6 s (2.5 s with a device) | 681-684 | host 721-736 | same |
| IPC post / pin / hot / dismissNewest (declines a ringing call) / holdAlerts / history / clearHistory / timeLeft | 687-724 | host 985-1028 | same |
| IPC next / previous (cycle the activity) | 693-694 | host 991-994 → `tell("menubar", {island})`; lib.rs `message` (1629-1643) only takes TrayMenu / M | missing: the message is dropped |
| IPC demo kinds incl. device pictures, `pictures`, `output`, `burst` | 726-777 | host 749-795 | different: no device pictures (Devices.pictureFor), no `pictures` demo |
| event / hasCard / ringingUp | 144-146, 180 | menubar island.rs:154-163 | same |

## Summary

1. `qs ipc call island next|previous` does nothing standalone (the bar drops the message).
2. Missing alerts standalone: new song, Pomodoro phase end, Focus on/off; and the "island said it" note that keeps a duplicate notification quiet.
3. Colour names / rgba strings in alerts (recording, charger removed) aren't parsed: wrong glyph colour.
4. History not persisted across host restarts; demo lacks device pictures.

## Fixed in batch 1 (fixer, 7 Nov)

- **Standalone live activities**: the host's Island service now derives recording, timer and stopwatch from the host's own Recorder (`recorder` state) and Timers (`timers` state) every turn (`derive_live`, called from `settled` and the 1 s tick), with LiveInfo's icon (record / pause-circle, timer / coffee), colour (red / grey paused, orange, green), title, pill time, detail ("24:59 left"), progress (timer) and every control (timer reset/play/skip via `nc` timer.*; stopwatch reset/lap/play via `nc` sw.*; recording discard/mute/pause via `recorder` IPC, stop via `screen.stopRecording`). Before, nobody posted them: no timer or recording ever showed in the island standalone.
- `push_live` also sends each activity's `detail`, `progress` and `all` (every control, with its piece), for the stacked rows and the open card; `island.live <kind> <i> all` runs one of every control (the pill keeps `<kind> <i>` over its shown pieces).
- An act can be an IPC call (`{"ipc": ["recorder","pause"]}`) as well as a host call.
- `qs ipc call island next|previous` now moves the focus (the menu bar dropped the message).
- Alert colours given as names ("danger", "orange"…) and `rgba(…)` are read (they fell back to the accent).
- **Running hot** (asking mode: "Cool down" / "Not now"): its buttons did nothing because the alert row's wheel TouchArea lay over the whole row and took every click (all alerts' buttons, not only this one). The wheel area is now under the row (`row := TouchArea` wrapping it); the buttons run `cooldown.answer true|false` → Cool-down → `island.hotAnswered` removes the alert; "Not now" sets Cool-down's `declined` and the alert stays away for the rest of the spell (it's posted only when `hot` turns on; `hot` stays on until the 60 s average is 5 °C under the threshold). A swipe/wheel up (accumulated, a notch) and the new close button (×, top-left of the glyph on hover, 20 px, 150 ms fade) remove it too (`island.remove`); the spell's later readings only update a removed uid (nothing comes back).

## Test plan: Running hot (Sonnet tester)

Arena with the user's settings plus `cooling.whenHot: true`, `cooling.hotAction: "ask"`, `island.hotTemp: 40` (so the sandbox's CPU counts as hot) and `temperature` in `island.events`; `--bin` the fixer's binary; both looks. If the sandbox has no temperature reading, feed one: `ipc` can't, so instead set `island.hotTemp` below the reading shown in the island's status line (hover the island to see `NN°`).
1. Wait up to 35 s: the island opens with "Running hot · CPU NN °C", a red ringed glyph, "Not now" (outlined) and "Cool down" (filled). `shot hot`.
2. Click "Not now": the alert leaves within ~0.3 s and does not come back for 2 minutes while the temperature stays high (shots every 30 s). Cool-down stays off (`dev run cat /tmp/irice-rt-4/...` not needed: the Control Center's Cool-down control is off).
3. End the spell: set `island.hotTemp: 99`, wait 65 s (it cools on the 60 s average), set it back to 40: a new spell posts the alert again. Click "Cool down": the alert leaves, Cool-down turns on (Control Center shows it; Material: power profile power-saver when ppd exists).
4. New spell again (as in 3). Hover the alert: a round × appears over the glyph's top-left (fades in ~150 ms). Click it: the alert leaves and stays away for the spell.
5. New spell again: scroll up over the alert (`P scroll` one notch up, or two-finger swipe up): it leaves.
6. With the alert up, any other alert's buttons work too: `ipc island demo download` then click "Open".

## Test plan: live activities (Sonnet tester)

1. `ipc timer toggle`: the island goes compact with the timer (icon, "Focus", 24:59 counting down, pieces per the user's livePieces: play, skip). Click pause in the pill (glide slowly onto it: the island must stay compact): the time stops. Skip: "Break".
2. `ipc timer stopwatch`: a stopwatch activity appears (green, counting up). Hover the island (activityMode all): rows for the timer and the stopwatch, with lap/play (stopwatch) and play/skip (timer); clicking each works.
3. Recording: `ipc recorder screen`: the recording activity (red pulsing dot, time). Open the island: its row (or card in one mode) has discard, mute (if a mic is set), pause (gpu-screen-recorder only) and a red stop; stop ends it ("Recording saved" alert).
4. `ipc island next` / `ipc island previous` with two activities in one-at-a-time mode: the pill switches.

## Test results

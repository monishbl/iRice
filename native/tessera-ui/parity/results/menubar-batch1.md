# Menubar parity, batch 1: test results (tester, arena 4, build 7ac31af)

Setup: user's config (Material dark, Colloid icons, expressive motion), `menubar.resources: true`,
`menubar.sound: true`, `island.events` without `resources` (the sandbox host load fires "CPU is busy"
alerts that hold the island open), hot-test runs with `cooling.whenHot`, `island.hotTemp: 40`.
Both looks (Glass: checked on Rust, QML twin only for the compact pill and the open island), QML twin
(`qml` pass) for every comparison below. Shots are in the tester's scratch dir
(`.../scratchpad/out/`: `r-m-*` Rust Material, `r-g-*` Rust Glass, `q-m-*`/`q-g-*` QML).

Harness lessons (important for the next tester): `vptr glide X Y MS` starts from the SCREEN CENTRE in
every new process, so use `P move X Y [sleep MS] [click|scroll|swipe]` in ONE vptr call instead
(a separate `P click` made the island close under the pointer). The island does not close when the
pointer leaves through a jump to empty desktop in either shell (no wl_pointer.leave); moving to
another point on the bar strip (1500,14) closes it.

## IslandStatus.qml: FAIL
- PASS  Icons not words: battery glyph + CPU chip + memory chip with `NN%` (Material: `r-m-status-open.png`; Glass: `r-g-open.png`).
- FAIL  Temperature figure missing. QML shows thermometer + `97°` between CPU and memory (`q-m-pop-cpu.png`, `q-g-open0.png`);
        Rust never shows it although the same machine has a reading (Rust's own popover says `97°C` under Processor).
        `resources.temperature` is read in lib.rs:1618 but nothing publishes that state key. Hot-red tint therefore untestable.
- FAIL  Island is taller than QML: open island with only the status line is 392x~82 (status line centre y=60) vs QML 392x~70 (status centre y=49).
        The figures sit lower and the pill has an extra ~11 px band.
- FAIL  Status line does not fade in / rise. Recording `r-m-status-in.mp4`: the figures are fully opaque from the first frame the pill
        grows and stick out under the pill's bottom edge while it is still short; QML fades them in (faint at the second frame).
        Also the pill opening lasts ~370 ms in Rust vs ~200 ms in QML (region diff of the island, `status-in` recordings).
- PASS  Hover CPU/memory 500 ms opens the popover under the figure, right row highlighted (CPU->Processor, memory->Memory), moving
        to the memory figure moves it. Timing: popover first frame 0.63 s after the pointer jump in both (Rust 1.483 s, QML 1.500 s into the recording).
- PASS  Leave: popover and island close (Rust popover closes ~30 ms after leaving, QML ~270 ms; Rust island starts closing ~170 ms vs QML ~380 ms: Rust is quicker than the 120/260 ms spec).
- n/a   Battery tip: not run (hover on battery figure).

## ResourceHover.qml: PASS (verified)
- PASS  Island figures: open 500 ms (parity with QML above), close on leave, new figure restarts, island closing closes it.
- PASS  Bar item (Material, `r-m-rh2-*` vs `q-m-rh2-*`): hover 1.3 s shows the popover under the item (top ~38 px), click removes it, and in Material no Resources menu opens in either shell (identical). Glass: click opens the CPU/Memory/Swap menu and the popover goes; hovering while open shows no popover; leaving closes (`r-g-rh-*`).
- Timeline (poll of the popover region): popover appears 0.87 s after the pointer starts moving onto the item.

## ResourcePopover.qml: FAIL
- FAIL  No fade/drop on opening: in `r-m-hover-cpu.mp4` the popover goes from nothing to fully opaque in ONE frame (region diff 0.02 -> 46.8);
        QML's rises over ~4 frames (8 -> 16 -> 37 -> 42, ~100 ms). Expected: fade + 6 px drop over Theme.motion.popover.
- PASS  Rows, badges, figures, graphs full on open, highlight on the right row, live updates, Material tones, Glass panel (`r-m-pop-cpu.png`, `r-g-pop-cpu.png` vs `q-m-pop-cpu.png`).
- note  Memory detail truncates to "6.4 of 15.3 GB · swap 2..." (QML "…· sw…" also truncates).

## IslandView.qml: FAIL
- PASS  Compact controls (Material and Glass): hovering prev/play/next keeps the pill compact, play pauses (`playerctl` Paused, icon becomes play, bars hide), next changes the track, clicking the title opens (`r-m-c*.png`, `r-g-ig-*.png`).
- PASS  Open card + rows (music card, Focus row with progress + pause/skip, Stopwatch row lap/pause, status line): `r-m-t2.png`.
- PASS  Timer pause freezes the time; stopwatch pause freezes; skip ends the focus (row leaves, history "Focus session done": QML identical, `q-m-k2.png`).
- PASS  Card prev/play/next: every click worked first time, 5 quick clicks on next = 5 track changes.
- FAIL  Recording stop does nothing. `ipc recorder screen` starts (alert "Recording", pill with red dot/time/stop, open row with discard + red stop).
        Clicking stop in the pill, clicking stop in the open row and `ipc recorder stop` all leave the recording running (timer kept counting 0:18 -> 12:46, `r-m-rr*.png`);
        a `wf-recorder -r 30` child of the arena stayed alive (even after kill -INT; I killed it with -9). QML: one click on stop ends it ("Recording saved" in Recent, `q-m-r2.png`).
- PASS  Pin: `ipc island pin` pins/unpins (Rust). Click on the pin and right-click not exercised separately on Rust (QML pin click verified: pins, stays, second click unpins and closes).
- PASS  Wheel in one-at-a-time (`island.activityMode: "one"`): scroll over the compact pill moves to the next activity (Focus -> music -> Focus), `r-m-w*.png`.
- FAIL  Glass open card layout differs from QML: Rust shows art + title/artist + seek bar in a row layout; QML (playerLayout overlay) shows a big centred glyph over title + "artist · album" + seek bar (`r-g-ig-open.png` vs `q-g-ig-open.png`).
- FAIL  Island open timing slower than QML (see IslandStatus: ~370 ms vs ~200 ms).
- n/a   Alert buttons are under IslandEvent. Space menu: right-click on an island dot shows a small hint "Only one display is connected" (`r-m-sp1.png`), QML not compared.

## IslandEvent.qml: FAIL
- PASS  `demo download`: hover shows the round x at the glyph's top-left; "Show in Folder" and "Open" both run and the alert leaves (`r-m-dl2-4.png`).
- PASS  `demo connect`: Connect -> "Connecting... keep the case open nearby".
- PASS  `notify-send -A ok=OK -A later=Later`: card (top-right toast, notifications are not in the user's island.events) shows both buttons, OK dismisses the card. (`notify-send --wait` printed nothing and stayed alive in BOTH shells, so the action delivery cannot be told apart.)
- PASS  `demo usb`: x dismisses; one wheel notch dismisses.
- FAIL  A tiny touchpad tick dismisses the alert: `swipe -4 40` over the alert 1.5 s after `demo usb` removed it in Rust (`r-m-usb7.png`, alert in Recent); the same swipe left it up in QML (`q-m-usb6.png`). Expected: only a deliberate swipe/notch dismisses.
- PASS  Running hot: see Island.qml.

## IslandActivity.qml: not verified
- PASS  Rows (timer, stopwatch, recording, music) appear in the open island with the right controls.
- n/a   The 45 ms stagger / 280 ms rise was not measured (rows were only checked statically; the status line itself has no fade, see IslandStatus).

## shell/services/Island.qml: FAIL
- PASS  Running hot (Material): alert "Running hot / CPU NN °C" with red ringed glyph, "Not now" (outlined) + "Cool down" (filled) appears within ~35 s of start (`r-m-hot1.png`); the NN updates live.
- PASS  "Not now": alert leaves at once, appears in Recent as "Running hot CPU 97°C", did not come back during 2.5 minutes (`r-m-n*.png`).
- PASS  Hover shows the round x at the glyph's top-left, click removes it; scroll up over the alert dismisses it; "Cool down" click removes the alert (`r-m-x*.png`, `s1`, `c1`). (Cool-down state in the Control Center and the power profile were not checked: no powerprofilesctl in the sandbox.)
- not run  Steps 3 (end spell, 65 s) and the second-spell variants beyond one restart each.
- FAIL  Temperature figure missing in the island status line (see IslandStatus), so "temperature turns red" cannot be seen.
- FAIL  Recording stop (see IslandView).
- PASS  Live activities: timer (compact pill, pause/skip), stopwatch (green, counting up), recording pill + row.
- not run  `ipc island next/previous` with two activities.

## BarMenu.qml: FAIL
- PASS  System/app/window menu open; click on the desktop/foot closes; click on empty bar closes (`r-m-bm1-3*.png`).
- FAIL  Step 4: with the system menu open, clicking the app name only CLOSES the menu; QML closes it and opens the app menu (`q-m-bm4-app.png`), second click toggles it closed.
        Rust: first click: nothing opens; second click opens (`r-m-bm4-app.png`, `r-m-bm4-toggled.png`).
- PASS  Rest 8 s on a row: stays open; leaving closes ~4.1 s after leaving in Rust (QML ~4.5 s): parity within half a second (poll files `r-m-auto-poll.txt`, `q-m-auto-poll.txt`).
- PASS  Keyboard: Down Down highlights the second item, Escape closes.
- not run  Tray (faketray: the icon is behind the collapse chevron; clicking the chevron only shows a "...r app icons · 3" tip in Rust, not compared with QML), space menu beyond the hint above.

## StatusMenus.qml: FAIL
- PASS  Sound menu opens under its item; closes on desktop click and bar click; Bluetooth menu opens; Escape closes (`r-m-sm*.png`).
- FAIL  Step 2: with Sound open, clicking the Battery item closes Sound and opens NOTHING in Rust (`r-m-sm2-battery.png`); QML opens the Battery menu (`q-m-sm2-battery.png`).
- PASS  Auto-close timing as in BarMenu.
- not run  Step 4 (Battery stays open 5 s after switching).

# Settings batch 1: tester results (arena 8, binary commit dc17dd2, 7 Nov 2026)

Shots/recordings (scratch, not committed): /tmp/claude-1000/s8/. Caveat: the sandbox recorder is variable-rate and the
page-switch tool's absolute offset moves between runs (0.2 s to 0.9 s), so only same-offset runs are compared
(noise about +-60 ms).

## Settings.qml
- T1 Material page switch: PASS (within noise). First change, QML runs 706/864 ms mean (821 excl. an outlier), Rust first visit 764/762 (885/870 excl. outlier); second visit 750 ms (<= first). Worst stall 0-17 ms (the 450-817 ms "stall" on click 1 is the Wallpaper thumbnails loading, same in QML: 867 ms).
- T1 Glass (G1): PASS. QML 861 ms vs Rust first visit 860 ms (same offset runs); second visit mean 70 ms (kept pages).
- T2 build times: PASS. Every first visit <= 20.2 ms (Displays 20.2, Desktop 15.3, rest < 4).
- T3 idle on Displays: PASS. 0 changed frames in 5 s (shots: t3-displays.png).
- T4 More options: PASS. 0 changed frames above the button; the added groups appear below (t4-after.png).
- T5 wallpaper thumbnails on return: PASS. Thumbnails present in the first frame back (t5-sheet.png).
- T6 Login Items: PASS. Page shown in 10.9 ms; first change 1.52 s then icons 1.82 s in the recording (QML 2.00 / 2.32 s, same 0.3 s icon fill).
- Observation (not in plan): Displays page, first visit in Rust with one active output: a "2" display rectangle is drawn over the page header and Identify row (t3-displays.png at 996,274) and "Connect another display to arrange them." replaces the QML's arrangement canvas panel; it was gone on a later visit. QML shows the canvas panel above Identify (qml-displays.png).
- Observation: Glass accent "following the wallpaper" is red in Rust, orange in QML (rust-glass-open.png vs qml-glass-open.png).

## SettingsMaterial.qml
- M1 pill spring: FAIL. Rust big motion about 400 ms then a 300 ms low-amplitude tail (burst 3.95-4.65 s and 5.95-6.72 s), QML about 140-220 ms (3.98-4.20, 5.93-6.15). Rust also records at about 20 fps during the move (frames every ~50 ms) vs QML 60 fps. It does overshoot and settle. Expect 250-400 ms, within 60 ms of QML.
- M2 cookie: PASS. Settings stats 41 frames in 3 s (<= 45; QML redraws all 60/s). Rotation not measured by eye.
- M3 keys: PASS. Click Appearance, Down, Down, End, Home: Wallpaper, Desktop & Widgets, About, Appearance (rust-m3-grid.png). (QML needs Tab first to focus the list; Rust works straight after the click.)
- M4 scroll indicator: PASS. Nothing idle, thumb after wheel, stays while the pointer is on the page, gone about 2.5 s after the pointer leaves (rust-m4.png). QML showed none in my 0.4 s shot (baseline unclear).
- M5: see Settings.qml.

## SettingsGlass.qml
- G1: PASS (see above).
- G2 search fold: FAIL. Scrolling the sidebar folds search to a magnifier, and a click opens it, but hovering the magnifier for 1.5 s does not open it (rg-g2-hover2.png). QML opens it on hover (qg-g2.png).
- G3 keys: PASS. Down, Down, End, Home: Wallpaper, Desktop & Widgets, About, Appearance.
- G4 scroll indicator: FAIL. The page thumb (and the sidebar's) stays visible 3.3 s after the pointer left the window (rg-g4cd.png); QML shows none (qg-g4.png). Material hides it correctly.

## ActionMenu.qml
- A1 opens as a popup: PASS. Under the control, right edges lined up, checked item marked; with the window moved to y=619 the list flips above the control and extends past the window edge (ra1-low.png). QML clips it at the window (qml-a1-low.png).
- A2 keys: FAIL. After opening, Down does not move the highlight: the key goes to the Choice in the window and toggles the list (first Down closed it, the second reopened it); Return then picks the unchanged item; Escape does not close the menu (Material rk.png/rust-a-sheet.png; Glass rg-a.png, rg-esc2c.png). QML: Down, Down, Return chose "Taps" (two below the checked item); Escape closed it and kept the window.
- A3 outside click: PASS. Menu closes and the click opens Focus, as the QML.
- A4 same control again: PASS (ra4.png).
- A5 long list (Default Apps Terminal): PASS for opening (2 items, below the control, checked "Automatic" marked: ra5c.png). The scroll-inside case was not reached (no list > 360 px; Style is 8 items = about 350 px).
- A6 look: FAIL. Material item pitch is 44 px in Rust vs 36 px measured in the QML shot (Off/Material/Marimba at y 630/666/702 vs 637/681/725), and the menu is wider in Rust (about 236 px vs 200 px). Rust Glass: the checked item has only a check mark, no accent fill, and no highlight for the pointer was seen.

## Choice.qml
- C1 caret: PASS. Turns over in about 200 ms on open (burst 1.62-1.83 s) and back on close (3.95-4.17 s).
- C2 toggle: PASS.
- C3 Down opens the list: PASS (a Down on the focused Choice opens it).

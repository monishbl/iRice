# Latency, stalls and reloads (parity-core)

The systemic reasons tessera-ui feels slower than the QML shell: how long a
panel takes from the command (a key, a click) to its first moving frame, how
long its animation lasts and whether it stutters, what holds up the UI
thread, and which lists or windows get rebuilt when nothing asked for it.

## How it is measured

`tools/live-scenarios.sh --suite latency` (this branch) runs every panel the
QML has, the way a person opens it, in a hidden arena with the user's
settings, once with the QML shell and once with tessera-ui standalone:

- **Keyboard shortcuts** as Hyprland runs them: `hyprctl dispatch
  'hl.dsp.global("tessera:<name>")'` (QML), and for the old Rust build
  `hl.dsp.exec_cmd("tessera-ui ipc call shortcut <name>")` (what
  keybinds.lua bound standalone until this batch). With this batch both
  shells use the global (tessera-ui registers the names itself), so the
  suite's Rust pass on the new binary should be switched to the global too:
  run it with `LAT_SC=global` (see the test plan).
- **Status menus**: `shell-ipc.sh call panels open <menu> TESSERA-TEST`;
  **clicks** on the menu bar (`ccClick`, `clockClick`).
- Each open and each close is recorded (wf-recorder, 60 fps). The pointer
  jumps 40 px in the bottom-right corner right before the command: that jump
  is the marker. `tools/anim-timing.py --marker` gives: ms from the marker to
  the first frame that changes elsewhere (open_ms / close_ms), how long the
  motion lasts (first burst of changes less than 250 ms apart), its frames,
  and the longest gap between two changing frames (a stutter shows here).
  Frame times are the encoder's (60 fps, constant): resolution 16.7 ms.
- `tools/latency-table.py OUT [OUT_BEFORE]` makes the table below (medians
  of the repetitions; LAT_REPS, default 3: the first repetition is the cold
  first open, so also look at rep 1 alone in latency-rust.tsv).
- `--perf` starts the Rust pass with TESSERA_PERF=1: the stall and reload
  log (below), copied per panel and repetition into `OUT/rust-perf.log`.

## The stall and reload log (TESSERA_PERF=1 or TESSERA_DEBUG=1)

core/src/perf.rs. Every piece of a loop turn that holds the UI thread more
than 16 ms is logged with its chain, innermost first:

    debug: stall 42.1 ms: msg host > ask panels.toggle > host Panels
    debug: stall 120.4 ms: command open controlcenter > part controlcenter
    debug: stall 30.2 ms: pointer press on tessera:menubar
    debug: stall 18.0 ms: render > render tessera:controlcenter
    debug: stall 25.3 ms: after_frame > part desktop

The pieces: each loop message (`msg <part>`, `msg data`, `line <first 60
chars>`), each part it reaches (`part <name>`), each host service and call
(`host <Service>`, `ask <call>`, `settled <Service>`), every IPC call
(`debug: ipc <args>`), pointer and key events by surface, Slint's timers,
each surface's render, each part's after_frame, each prewarm
(`debug: prewarm <part> N ms`).

Reloads: `debug: reload list <surface>.<list> (N rows)` when a surface hands
a list a new model on a refresh, `debug: reload window <surface>` when it
makes a window again. SIGUSR1 also prints `{"reloads":N,"stalls":N}`.

## Before (main at 0b96f00, `target/release/tessera-ui.before`)

Measured in arena 3 before the protocol change (fixers no longer run
sandboxes), Glass look (the user's config.json had `look: glass` then), 1
repetition, so rough:

| what | Rust before | QML |
|---|---|---|
| `tessera-ui ipc call shortcut <x>` (client, per key press) | median 11–29 ms (3 runs: 29.4, 13.9, 11.3 ms; min 8, max 47) | 0 (no process: global shortcut) |
| Control Center by shortcut, first open after start | 533 ms to first motion (one run); in a second run it hadn't moved 1.6 s after the shortcut | not measured yet |
| Sound menu by IPC | 533 ms | not measured yet |
| Control Center close | 433–483 ms to first motion, 83–183 ms of motion, longest gap 33–67 ms | not measured yet |

The full before/after table is the tester's first job (test plan, step 2).

## What this batch changed (commits on worktree-agent-ada0b8b5ee0fdea1d)

1. **Keyboard shortcuts without a process per key press**
   (core/src/global_shortcuts.rs, host/shell/src/shortcuts.rs,
   hypr/tessera/keybinds.lua). tessera-ui registers every shortcut name as a
   Hyprland global shortcut (`tessera:<name>`, hyprland-global-shortcuts-v1,
   exactly what Quickshell does for the QML's GlobalShortcut items) on a
   connection of its own; keybinds.lua binds `hl.dsp.global` for both
   shells. Before, standalone ran `tessera-ui ipc call shortcut <name>` for
   every key press: the whole 90 MB binary started, connected, waited for
   the loop's answer (8–47 ms per press, more under load). Worse: the drop
   shelf's `pointerDown`/`pointerUp` binds are on the left mouse button, so
   **every click anywhere started two tessera-ui processes**. gestures.lua
   and findpointer.lua already used `hl.dsp.global`: standalone, nobody
   answered them until now. A test checks every QML GlobalShortcut name is
   registered.
2. **Panels made ahead** (`Part::prewarm`, src/main.rs): 2 s after the first
   frame, one part per quiet loop turn, the windows the Rust keeps while
   closed are made and left unmapped: Control Center, Spotlight,
   Notification Center, Overview, Mission Control, the switcher, the power
   menu. Their first open no longer builds the component and its surface.
3. **No picture decoding on the loop in Control Center**: `picture()` keeps
   what it decoded by path and file time (the sound page drew its device
   SVGs with resvg on every refresh); ~/.face is decoded on a thread
   (M::Avatar) instead of on every open.
4. **The host's state goes to the parts as data, not as a JSON line written
   and read back** (core/src/host.rs: Msg::Data(ShellState) instead of
   Msg::Line(text)); every line from the host/shell is parsed once, not up
   to three times (src/main.rs shell_line, Command::from_value,
   bridge::from_value).
5. **after_frame**: glass shapes compared by a hash of their numbers
   (`theme::glass_key`), not `format!("{shapes:?}")` (every float printed,
   every frame, every part) — 28 sites in the surfaces this agent owns.
6. **Rows** (lists changed in place) in the surfaces this agent owns: see
   the commit and the list in the report.
7. The stall and reload log (above).

## Known suspects for the tester's log (not changed yet)

- Control Center close stutter: when it closes, `shell::closed` makes the
  host publish the state; every part's `data(ShellState)` runs while the
  close animation plays (menubar, statusmenus, desktop, dock read it: other
  agents' crates), and the keyboard going back to an app makes Hyprland's
  snapshot change (every part's `data(Snapshot)`). If the log shows
  `msg data > part menubar|desktop|dock` stalls during the close, those
  crates must diff instead of rebuild (Rows; the menubar/desktop agents).
- `Glass::clear_now` is a blocking Hyprland socket request (CC, spotlight,
  NC, switcher when the panel moves to another output).
- `clipboard_text` (Ctrl+V in a text field) runs `wl-paste` synchronously
  (core/src/wayland.rs).

## Test plan (for the tester; arena of your own, never the host's screen)

Binaries: after = `<worktree>/native/tessera-ui/target/release/tessera-ui`,
before = `<worktree>/native/tessera-ui/target/release/tessera-ui.before`.
Run everything from the worktree (`tools/live-scenarios.sh` and
`tools/anim-timing.py` are this branch's), with the user's config (the
default `--config`). Do both looks: the user's (`--look material`, as the
user runs it now) and `--look glass`.

1. **Shortcuts reach the new binary.** Bring an arena up with the after
   binary, standalone (`KEEP_UP=1 LAT_ONLY=none tools/live-scenarios.sh
   --arena N --suite latency --only rust --perf --look material --out OUT0`).
   In the log (`.sandbox/arena-N/home/.local/state/tessera/tessera-ui.log`)
   there must be no "no global shortcuts" line. Then `./dev.sh hyprctl
   dispatch 'hl.dsp.global("tessera:controlCenter")'` must open the Control
   Center (`./dev.sh shot`), and again close it; same for `spotlight`,
   `overview`, `launchpad`. Press ALT+Space and the arena's other binds with
   `./dev.sh run wtype …` (the arena's mod key is ALT; binds from
   hypr/tessera/keybinds.lua): they must act. Left clicks on the desktop
   must not start any `tessera-ui ipc` process (`ps -eo args | grep "ipc
   call"` while clicking: nothing). PASS/FAIL each.
   NOTE: the suite's Rust pass dispatches `exec_cmd(… ipc call shortcut …)`
   (the old path, to measure the before binary). For the after binary set
   `LAT_SC=global` so it dispatches `hl.dsp.global` like the QML pass (the
   fixer added the switch: sc_cmd in live-scenarios.sh).
2. **The latency table, before and after.** Three runs, each LAT_REPS=3:
   - `tools/live-scenarios.sh --arena N --suite latency --perf --look material --out OUT_after` (QML and Rust after, LAT_SC=global)
   - `tools/live-scenarios.sh --arena N --suite latency --only rust --look material --bin …/tessera-ui.before --out OUT_before`
   - `python3 tools/latency-table.py OUT_after OUT_before` → paste under
     "## Results" below with the date; also rep 1 (cold) for each panel
     from `latency-rust.tsv`.
   Then the same with `--look glass` (a second table).
   PASS per panel when Rust open_ms ≤ QML open_ms (warm, median) and the
   animation length is within ±10 % of the QML's and no Rust gap > 2× the
   QML's. First check with `./dev.sh shot` that the click coordinates in
   LAT_PANELS (ccClick 1753,14; clockClick 1850,14) hit the Control Center
   item and the clock in this arena at 1x; fix them in the script if not.
3. **Stalls.** In `OUT_after/rust-perf.log`: list every `debug: stall` line
   (panel, repetition, ms, chain). Any stall during an open or a close is a
   FAIL to report with its chain (the fixer's next batch removes them).
4. **Control Center close stutter.** Both looks: open the Control Center by
   its shortcut, wait 1 s, close it by its shortcut, recorded
   (`lat_rec` does this: the `controlCenter` close row). Report
   close_gap_ms QML vs Rust and the stall lines logged during the close.
   PASS when the Rust's longest gap ≤ the QML's + 17 ms and no stall.
5. **Prewarm.** Right after the arena starts (TESSERA_PERF), wait 5 s: the
   log has `debug: prewarm <part> N ms` lines (each part once; note any
   > 50 ms). Then without opening anything, take `./dev.sh shot`: nothing
   new on screen, and a click in the middle of the screen still reaches the
   desktop (a desktop widget's hover, or a right-click menu on the
   wallpaper) — a prewarmed full-screen panel must not swallow clicks.
   Then the first open of Control Center / Spotlight / Notification Center
   / Overview / Mission Control: rep 1 of the latency suite ≤ rep 2–3 + 1
   frame.
6. **Reloads.** With TESSERA_PERF, in the arena: open two `foot` windows,
   close one (`hyprctl dispatch 'hl.dsp.window.close({ window = "class:foot" })'`),
   focus the other (click it), `notify-send -a Files 'Copied' '3 files'`,
   open and close the Notification Center, Overview, Mission Control.
   `grep "debug: reload" tessera-ui.log` must be empty for every surface of
   this agent (cheatsheet, clipboard, controlcenter, emoji, missioncontrol,
   network, notes, notifications, osk, overlay, overview, screentools,
   session, settings, share, shelf, sidebar, spotlight, switcher, tabs…);
   list any line (other agents' surfaces: report, not a FAIL here).
7. **Control Center avatar and sound page.** Open the Control Center: the
   avatar shows (after at most a moment on the first open); open the Sound
   page twice: device pictures show both times, and the second time no
   `stall` names `controlcenter`.

## Batch 2 (what changed)

1. **Readings held back while a window animates** (core/src/runloop.rs,
   Shell::deferred). A service's reading (`Msg::Data`: the host's state,
   Hyprland's snapshot, sound, battery…) went to every part's `data()` in the
   turn it arrived; closing a panel makes two at once (the state says it
   closed; the keyboard goes back to an app: a new snapshot), so the closing
   frame waited for every part. Now, while any window runs an animation, each
   part's call is queued and handed out between frames, ~4 ms per turn, and
   all at once as soon as nothing animates. Each part still gets every kind
   of reading, the newest. The stall log names them `deferred data > part <name>`.
2. **Lists inside rows** (`tessera_core::rows::shared`): Slint compares models
   by identity, so a row holding a list made afresh never equalled its old
   self and `Rows::set` changed every such row on every refresh (its inner
   list made again: a card's buttons lost their hover). Equal contents now get
   the same model: notification cards' buttons, the widgets' colours and
   lines, Control Center's block options, the sidebar's Markdown blocks,
   Overview's zone chips, Live Text boxes, cheatsheet groups and keys, emoji
   rows, Settings' options, the share card's buttons, the editor's width tips,
   the on-screen keyboard (it rebuilt every key on Shift).
3. **Start-up**: `udevadm --version` (the mouse service) asked on its thread
   instead of the loop; every part and host service taking 25 ms or more to
   start is logged (`tessera-ui: slow start: <part> N ms`), beside `started
   every part N ms after main` and `first-frame … since-main-ms=N`. The panels
   made ahead start 2 s after the first frame, one per quiet turn, never before it.
4. **Theme**: `Theme.expressive` (appearance.motion, "expressive" by default),
   TextButton's corners and press swell as the QML's, `clover8` among the shapes.

5. **Big windows draw only what changed** (core/src/egl.rs, vendor/femtovg).
   FemtoVG drew and filled the whole window every frame (damage only told
   the compositor); a large window with a small animation (Settings' sidebar
   pill) filled millions of pixels per frame. Now, with EGL_EXT_buffer_age,
   each frame is scissored to the box that changed since its back buffer was
   last drawn (the vendored femtovg 0.27.0 has one change: a screen clip its
   GL renderer honours for screen drawing and clears, never for layers).
   TESSERA_UI_FULL_REDRAW=1 turns it off (to compare, or if anything is
   left stale). The stall log splits a slow frame into `damage probe <ns>`
   and `femtovg draw <ns>`; `TESSERA_DEBUG` logs whether the driver has
   buffer age (`debug: egl: damage true buffer age true`).
6. **Wallpaper accent as the QML picks it** (core/src/seed.rs, theme.rs):
   the tone is Quickshell's ColorQuantizer (median cut, depth 3, 64 px) and
   pickAccent, snapped by HSV hue in Wallpaper.qml's order; grey is blue.
   It was Material's seed snapped by HCT hue (red where the QML said orange).

### Test plan, batch 2

8. **Control Center close, again** (step 4 with this binary): both looks, 5
   repetitions; close_gap_ms and the stall lines during the close
   (`deferred data` lines are expected; PASS when none is over 16 ms and the
   Rust's longest gap ≤ the QML's + 17 ms).
9. **Readings still arrive**: while the Control Center closes, change the
   volume (`wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+`) and send a
   notification: the menu bar's sound icon and the banner follow right after
   the close (≤ 0.5 s). The menu bar's Control Center item stops looking
   pressed when the panel closes.
10. **Hovers survive refreshes**: hover a notification card's action button
    in the Notification Center and wait 70 s (the minute tick): the hover
    stays. On the on-screen keyboard press Shift: keys change their labels
    without flashing.
11. **Start-up**: restart the shell 5 times (`./dev.sh reload`); from the log
    report `started every part … ms after main`, `first-frame …
    since-main-ms=…` and every `slow start` line, both looks. PASS: first
    frame < 1000 ms on every start; otherwise list the slow parts.
12. **Settings at 60 fps**: open Settings (a big window), record its sidebar
    pill moving between pages (click page after page), both looks; frames
    and longest gap from anim-timing.py, QML vs Rust; then the same with
    TESSERA_UI_FULL_REDRAW=1 (the old way) to show the difference. With
    TESSERA_PERF, list any `femtovg draw` / `damage probe` stall lines.
    Look closely for stale pixels: hover rows, scroll a page, open a menu,
    type in a field, resize Settings: nothing left behind (shadows' edges,
    a caret, a hover tint). PASS: ≥ 55 fps, no stale pixels.
13. **Accent**: with the same wallpaper and `appearance.accent: "wallpaper"`
    in Glass, QML and Rust show the same accent (Overview, Mission Control,
    Settings' accent swatch). Try 3 wallpapers (the bundled default, a
    warm one, a grey one).
15. **Material colours**: the scheme itself matches the QML's exactly (the
    fixer compared every role of `tessera-ui --material-roles SEED dark|light
    tonalSpot` with the QML's MaterialColor.js for four seeds: 53 of 53 equal),
    and now follows Settings' contrast as Theme.qml does (it was always
    standard). So a different container colour means a different seed: with
    the wallpaper the arena shows, run `matugen image <picture> --json hex
    --dry-run --source-color-index 0` (the QML's seed) and `tessera-ui --seed
    <picture>` (the Rust's) and report both, and the picture each shell
    uses (QML: Wallpaper.path; Rust: `wallpaper::showing_now`, the log's
    seed line with TESSERA_DEBUG). Then Spotlight's Calculator hero and Top
    hit: which role each draws with, QML vs Rust (the Spotlight fixer's).
16. **Motion token**: `appearance.motion` subtle / snappy / expressive in
    Glass: surfaces that read `Theme.expressive` (list from the fixers) move
    as the QML does; Material always expressive. TextButton (empty and error
    states' buttons: e.g. Clipboard with nothing in it): Glass presses in to
    97 %, Material swells and its corners square off.

## Batch 3 (what changed)

1. **Touch, shell-wide** (core/src/touch.rs). The seat's touch is bound in
   core and the first finger is the pointer, as Qt synthesizes mouse events
   for the QML's items: a tap is a left click where the finger went down; a
   finger moved past 10 px presses the left button where it went down and
   drags (Flickables scroll and fling, sliders slide, drags drag); held still
   500 ms it is a right click (context menus); a compositor cancel lifts the
   button outside, clicking nothing. Other fingers are ignored meanwhile.
   (The dock's own wl_touch binding must go when this merges: two bindings
   would each get the events.)
2. **Partial redraw's safety net** (core/src/damage.rs, shell.rs): a frame
   is drawn whole when the look changes (config), when the glass plugin
   comes or goes, when a popup opens or closes inside a window (the number
   of components drawn changed), when two thirds or more of the window
   changed anyway, and, as before, on the first frame, a new size or scale,
   or an unknown buffer age.

### Test plan, batch 3

17. **Touch** (the arena's virtual touch, if the sandbox has one — else a
    real touchscreen session, skip and say so): on the dock tap an app (opens),
    long-press an app (its menu), drag an icon; in the Control Center tap a
    toggle, drag a slider; in the Notification Center flick the list; in
    Launchpad flick between pages; in Settings tap a page, scroll a page by
    dragging, long-press nothing (nothing happens). Two fingers at once: only
    the first acts. PASS each as the QML does.
18. **Partial redraw after changes**: with Settings open, switch the look
    Glass ↔ Material and dark ↔ light from Settings itself, open and close a
    menu inside Settings and the dock's context menu, resize Settings: no
    stale pixels anywhere afterwards (shots before/after of the same state).

## Results

(The tester writes here: date, binary, look, the table, PASS/FAIL per step.)

## Step files

The test plans above as runnable steps (one action per line, top to bottom
from a fresh desktop): `parity/steps/` — latency-panels(-glass), shortcuts,
cc-close, reloads, prewarm, hovers, settings-redraw, colors, motion,
startup, touch; the icon checks in `icons.steps`.

## Lists in place, the remaining surfaces (12 Nov)

- dock, launchpad: no model is replaced on refresh (all Rows / rows::shared).
- desktop: only the widget gallery's kinds, set once when it opens.
- menubar: the workspace chips' app icons and the menu rows' tiles were new
  models on every strip change / live menu update, so each chip's icons were
  rebuilt; now `rows::shared` (the same model for the same items).

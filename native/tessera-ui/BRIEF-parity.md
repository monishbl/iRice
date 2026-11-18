# Parity round: make tessera-ui as good as the QML shell, in use

tessera-ui (Rust + Slint) replaced the QML shell (shell/, Quickshell) as the
user's live shell on 7 Nov 2026. Static screenshots matched, but in use the
user found it "worse than QML in every aspect other than resource efficiency":

- icons not loading (somewhere: find where; the user runs Material dark with
  the Colloid icon theme)
- closing a window reloads all the desktop widgets
- animations feel slower, everything feels slow (we measured ~380 ms more
  before the Control Center starts to open; animations themselves run at 60 fps)
- the island said "CPU, memory, battery" as words instead of icons, and
  hover popovers don't work
- "it's like the port is only 30% proper"

The verification so far compared still pictures of states driven by IPC.
That missed behaviour. This round is about behaviour: every hover, click,
right-click, drag, scroll, key, timer, animation (duration, easing, what moves)
and live update the QML does, the Rust must do the same way.

## How the work is split (from 7 Nov, the user's choice)

- **Fixers (Opus)**: read the QML, write the parity file (behaviours AND a
  test plan: the exact steps a tester runs — pointer moves/glides/clicks
  with coordinates, keys, what to record, what must be seen, and the QML
  timing to match), fix the Rust, build. **Fixers don't run sandboxes.**
  Mark `written` then `built`. Work in batches (5–10 QML files); when a batch
  is built and committed on your branch, stop and report (files, commit,
  binary path). The lead starts a tester on it and resumes you with the
  next batch and the tester's failures.
- **Testers (Sonnet)**: take a built batch, run each parity file's test
  plan in their arena with the user's settings against the fixer's binary
  (`tools/live-scenarios.sh --bin <worktree>/native/tessera-ui/target/release/tessera-ui --arena N`,
  extended per plan; both looks), and for each behaviour write PASS or FAIL
  with what was seen (and recording timings) under "## Test results" in the
  parity file. Testers never edit code. Mark `verified` only when every
  behaviour in the file passed; otherwise leave it `built`.

## What "done" means for a QML file (your checklist item)

1. **Read the QML file fully** (and the Material twin: Material*.qml next to
   it, or material/ overrides, when there is one). List every behaviour: each
   MouseArea/HoverHandler/TapHandler/DragHandler/WheelHandler and what it does,
   each Keys handler, each Timer, each Behavior/animation (property, duration,
   easing/spring), each popover/tooltip/menu it opens, each state, each live
   binding to a service (what updates it and when), each Loader (lazy or kept).
2. **Find the Rust that does it** (surfaces/<x>, host/<x>, core). Mark each
   behaviour: same / missing / different. Write the list to
   `native/tessera-ui/parity/<qml path with / as _>.md` (one file per QML
   file: the behaviour, its QML line, its Rust place, status).
3. **Fix every missing and different one.** No "Not like the QML" shortcuts:
   if Slint truly can't (custom shaders, variable-font axes), do the closest
   thing that looks the same at a glance and say exactly what differs in the
   parity file. A plain text line where the QML has icons, figures and hover
   popovers is NOT acceptable.
4. **(Testers) verify in a sandbox by doing it**, both looks (Glass and Material), with
   the user's settings (see below): move the pointer, hover, click, type,
   drag, open and close windows; record and time animations against the QML
   doing the same (tools/live-scenarios.sh shows how: copy and extend it for
   your scenarios). Then mark the item verified in the tracker.

Tracker (deterministic progress, the user watches it):
  python3 ~/.local/share/tessera-port/prog.py set <group> <qml path> written|built|verified
  python3 ~/.local/share/tessera-port/prog.py doing <group> <what now, short>
Set `written` once the parity file is done and fixes are in, `built` once it
builds with them, `verified` only after you saw it behave like the QML in the
sandbox (pointer/keyboard, both looks).

## Systemic rules (apply everywhere you touch)

- **Lists change in place.** Never `set_x(ModelRc::new(VecModel::from(..)))`
  on a refresh: keep a `tessera_core::rows::Rows<T>` per list, hand
  `rows.model()` to the component once, then `rows.set(v)` (by position) or
  `rows.set_keyed(v, |r| key)` (rows that move: notifications, windows).
  A model replaced on refresh rebuilds every element (the widgets "reload").
- **Don't rebuild a window to update it.** Keep windows made (hidden, not
  dropped) when the QML keeps its Loader active; make them ahead (at start or
  idle) when the QML has them ready, so opening is instant. Measure the time
  from command to the first frame that moves (tools/anim-timing.py on a
  recording) for both shells: Rust must not be slower.
- **No per-frame work that isn't needed.** `after_frame` runs after every
  frame of every window: compute glass shapes etc. only when their inputs
  changed.
- Same durations, easings and springs as the QML (Qt's SpringAnimation
  spring/damping/epsilon ≠ Slint's spring(): match the curve by measuring).

## The sandbox, with the user's settings

- Your own arena (given in your brief): `TESSERA_ARENA=N ./dev.sh up/down/reload/shot/run/pointer`.
  `tools/live-scenarios.sh --arena N` sets an arena up with the user's
  config.json (Material dark, Colloid icons, expressive motion, their widgets;
  accounts and keys left out) and runs both shells. Copy it to your scratch
  dir and add your scenarios; run `--only qml` / `--only rust`.
- Pointer: `TESSERA_ARENA=N ./dev.sh pointer -o TESSERA-TEST move X Y | glide X Y MS | click | down | up | scroll`
  (glides produce hover enter/leave; jumps may not). Each vptr process starts
  at the screen centre, so put a whole path in ONE call
  (`P move X Y sleep 400 move X2 Y2 click`), not one call per step.
  Keys: `./dev.sh run wtype -d 30 …` (the default speed can drop keys).
  Run at most 2 tester arenas at once (each nested Hyprland costs a core;
  with builds the host load passed 40 and pointer steps took minutes).
  The arena's mod key is ALT.
- Timing: `python3 tools/anim-timing.py rec.mp4` (start, end, frames, longest
  gap); `python3 tools/region-changes.py rec.mp4 x y w h` (did this area
  change: a widget reloading).
- Also check Glass: `use_config` in the script can set appearance.look.

## Build and test

Build only through the gate, in your worktree, with its own target dir:
  cd native/tessera-ui && CARGO_TARGET_DIR=$PWD/target SLOT_JOBS=2 SLOT_MEM_HIGH=5G SLOT_MEM_MAX=6G RUST_MIN_STACK=67108864 ~/.local/share/tessera-port/slot.sh cargo build --release -p tessera-ui
Tests: `… cargo test --release -p <crate>`; python: `python3 -m pytest -q tests/test_rust_ui.py`.
Build when you need to check behaviour, not after every edit.

## Never

- Never run tessera-ui, tests or anything graphical with the host's display
  (outside dev.sh: `env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET -u DISPLAY`).
- Never screenshot or record the host; only `./dev.sh shot` / wf-recorder
  inside an arena on TESSERA-TEST.
- Never pkill/killall (the container shares PIDs with the user's desktop,
  whose shell IS tessera-ui): kill exact PIDs after checking
  /proc/PID/environ has your arena's XDG_RUNTIME_DIR. Never run
  repair-shell.sh, update-run.sh or anything that looks for "the shell"
  from inside an arena (it can see and stop the user's live one).
- Never touch ~/tessera (the user's live install) or arena 1, 6, 7.
- Never set TESSERA_VISIBLE=1 or TESSERA_HOST_AUDIO=1. Arenas stay hidden;
  `./dev.sh down` when you finish.
- Never commit screenshots/recordings. No company trademarks in UI text.
- Commit on your worktree branch; messages end with
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

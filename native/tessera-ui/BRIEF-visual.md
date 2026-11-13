# Visual parity round: the Rust shell must look like the QML shell

Branch: `rust-rewrite`. The reference is the last all-QML version (342a6ac,
checked out read-only at `~/tessera-verify/qml-ref`): its `shell/` is what
the QML screenshots show. Read the QML there for exact sizes, colours,
radii, spacing and states; don't edit any QML.

The user's bar: not literally pixel-perfect, but **nothing broken, missing,
overflowing, misaligned or clearly different**. Same layout, same elements,
same colours and tints, same glass (frost, rim, shadow), same icons, same
states, in both looks (Glass and Material).

## The screenshots

`~/tessera-verify/visual-rr-3/` holds the latest full capture, made with the
user's own settings and wallpaper in hidden sandboxes:
- `qml/<look>-<screen>.png` and `rust/<look>-<screen>.png` (1920×1080),
  `diff/…` (changed pixels in red), `index.html` (slider, side by side, diff)
- the screens and how each is opened: `tools/visual-scenes.txt`

Look at pairs yourself: crop the region that matters and view QML | Rust,
e.g.
    magick qml/glass-settings-sound.png -crop 1060x720+430+190 +repage /tmp/q.png
    magick rust/glass-settings-sound.png -crop 1060x720+430+190 +repage /tmp/r.png
    magick /tmp/q.png /tmp/r.png +append -resize 1400x /tmp/pair.png
(then Read /tmp/pair.png; use your scratchpad, not /tmp, if you have one)

**Ignore** (not differences): time and date, battery %, Wi-Fi/Bluetooth
names, network lists' contents, notification and history counts, weather
numbers, which page a rotating widget stack shows, slightly heavier or
softer text (the renderer, parked by the user), a mouse pointer.
**Fix** everything else: positions, sizes, alignment, wrapping, overflow,
missing or extra elements, colours and tints, borders and rims, shadows,
radii, icons and icon styles, empty states, wrong values shown.

## Traps already hit (read before changing anything)

- Slint: an element with a size but no x/y is **centred** in its parent; a
  Text left to size itself takes its one-line width (pin x/y/width/height).
- Slint layouts **stretch by default** (horizontal-stretch 1): an empty slot
  layout takes spare width; give fixed slots `horizontal-stretch: 0`.
- A wrapped Text in a nested layout is measured at the wrong width; the
  Settings row measures a hidden copy at the real column width (see RowView).
- Glass with the plugin (`Theme.plugin`): the plugin draws the glass, its
  tint and its rim. Don't draw another tint or rim over a glass shape (a
  circular Slint rim beside the plugin's squircle reads as a double edge; a
  tint makes it darker than the QML). Without the plugin, keep the fallback.
- Windows made ahead (prewarm/Prepare/make_ahead) must `hide()` at once.
- Lists change in place (`tessera_core::rows::Rows`, `rows::shared`).
- Custom Settings controls that size themselves belong inside the row
  layout (`inline` in settings.slint); whole-row ones outside.

## A worktree needs three links first

A fresh worktree lacks three git-ignored things the sandbox needs; without
them captures fail ("NO SHOT") or Glass renders without the plugin (dark,
opaque: misleading). Link them from the main checkout before capturing:
    M=/home/user/Projects/Personal/iRice
    mkdir -p .sandbox && ln -sfn $M/.sandbox/home .sandbox/home && ln -sfn $M/.sandbox/bin .sandbox/bin
    ln -sfn $M/hypr/plugin/tessera-glass/tessera-glass.so hypr/plugin/tessera-glass/tessera-glass.so
Check your branch is on rust-rewrite (`git merge-base --is-ancestor rust-rewrite HEAD`).
Worktree agents share one scratchpad: give logs and images unique names.

## Build and check (others build and capture at the same time)

Before your first build, seed your target dir from the main checkout's full
build (btrfs copy-on-write: seconds, no extra disk; a fresh target compiles
every dependency, ~40 min with the memory shared):
    cp -a --reflink=always /home/user/Projects/Personal/iRice/native/tessera-ui/target/release native/tessera-ui/target/
Build in your worktree, through the gate, with your own target dir:
    cd native/tessera-ui && CARGO_TARGET_DIR=$PWD/target SLOT_MEM_MAX=8G RUST_MIN_STACK=67108864 ~/.local/share/tessera-port/slot.sh cargo build --release -p tessera-ui
The gate is a shared memory budget: it starts your build when the memory
fits (first come, first served), picks the jobs from the memory granted (all
cores when yours runs alone) and shares compiled dependencies through
sccache. Don't set SLOT_JOBS or bypass it; a wait message is normal.

Check your screens against the same QML shots, in **your own arena** only:
    tools/visual-compare.sh --bin $PWD/native/tessera-ui/target/release/tessera-ui \
        --sets 1 --rust-arenas <YOUR ARENA> --reuse-qml ~/tessera-verify/visual-rr-3 \
        --only '<glob, e.g. settings-*>' --out ~/tessera-verify/<area>-<n>
(one glob per run; both looks; then look at the pairs in that folder)
Capture after a batch of fixes, not after every edit.

## Never

- Never touch the user's live session (~/iRice, their screen, arena 1); the
  laptop runs the QML shell and stays on it.
- Never pkill/killall (shared PIDs): exact PIDs only, checked first.
- Never use another fixer's arena. No sub-agents.
- Don't change core/ (shared by every surface) unless your area can't be
  fixed otherwise; if you do, keep it minimal and say exactly what in your
  report (the lead merges everyone's work).
- Commit on your branch (messages end with
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`); never commit
  screenshots.

## Report when done

Per screen (both looks): matches / fixed (what) / still differs (what and
why). Then your commits, and any core/ change.

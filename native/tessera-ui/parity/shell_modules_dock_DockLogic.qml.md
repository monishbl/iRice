# shell/modules/dock/DockLogic.qml → surfaces/dock/src/{model.rs,layout.rs,lib.rs}

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Items: Apps button, pinned then running apps; recent apps (3); minimized windows, phone, Downloads, Trash; a divider on each later section's first item | 42-53 | `model::build` | same (no phone item: the Phone service isn't in Rust) |
| Delegates aren't rebuilt when only the recents' order changes; same ids with new entries refresh the icons (shownItems/shownKey) | 54-70 | slots are a `Rows<DockSlot>` handed to the window once, `set_keyed` by `kind:id|` (DockSlot.key): unchanged items keep their element, an app opening or closing inserts/removes only its icon. Before, rows were overwritten by position: after a window closed every later icon's row changed, **and its `bounce` counter moved to its neighbour (the wrong icon hopped)** | fixed |
| Icons refresh when the app list reloads | 61-64 | `data(Apps / Icons)` clears the path cache, `rebuild` | same |
| One passive pointer tracker over the window; hover decided from fixed rectangles | 73-83 | dock.slint `touch` TouchArea → `pointer` → `layout::hit` | same |
| Touchscreen: a DragHandler (TouchScreen only, y axis) on the window; translation.y < −24 while auto-hiding → touchReveal: revealed, touchHold 4 s, then hidden unless the pointer wants it up; hideDelay waits while touchHold runs | 84-89, 108, 110-120 | **was missing (core binds no wl_touch, so touches reached nothing)**. surfaces/dock/src/touch.rs binds the seat's wl_touch itself (its own wl_registry + wl_seat on the shell's connection; the Dispatch impls live in the dock crate). Dock window: `Dock::touch` — a finger's first point; past 24 px up while `auto` → `Reveal::touch()`, `touch_hold` 4 s → `Reveal::hold_over()`; `M::Hide` skipped while `touch_hold` runs | fixed |
| Touch on the icons: Qt makes mouse events from touches, so a tap is a MouseArea click | DockItem 326-335 | core's touch (core/src/touch.rs: the first finger as left press / move / release, a 500 ms hold as a right click) reaches the dock's TouchArea like the mouse; the dock's own tap handling was removed so a tap clicks once | same (via core) |
| No hover from touches (HoverHandler ignores the touchscreen) | DockItem | core moves Slint's pointer to the finger ("so hovers show") and sends PointerExited after the lift: while a finger is down on the dock it magnifies / shows the label under it, then stops | different: core's choice for all surfaces; the dock can't tell core's synthesized pointer from the mouse |
| Touch on the menu and the Downloads stack (their MouseAreas, via Qt's synthesized mouse) | DockMenu / DownloadsStack | core's touch (the dock no longer forwards these itself: one touch acts once) | same (via core) |
| Touchpad flicks / momentum | — | nothing in the dock's QML scrolls (no Flickable, ListView or WheelHandler in shell/modules/dock: the Downloads stack shows at most 12 files in a fixed grid), so there's no fling to match | same (n/a) |
| Auto-hide: always / when a window covers the resting pill / off | 92-93 | `layout::auto_hiding`, `windows.covered(cover_rect)` | same |
| Hidden = autoHiding && !revealed && no menu && no stack | 95 | `layout::hidden(auto, revealed, popup)` | same |
| Reveal after 120 ms on the 2 px edge; hide 300 ms after leaving the zone | 96-109 | `layout::Reveal`, `REVEAL_DELAY_MS` / `HIDE_DELAY_MS`, `timers()` | same |
| Input mask: edge strip while auto-hiding/hidden, the zone once revealed, else the pill's largest extent | 126-146 | `layout::input` → `set_input_region` (sent when changed) | same |
| Opening any panel closes the menu and the Downloads stack | 148-155 | stack: `DownloadsStack::data(ShellState)`; menu: **was missing**, now `Dock::data(ShellState)` closes an open menu when `open` changes from what it was when the menu opened | fixed |

## Test plan

### Geometry (the user's settings, 1920x1080 TESSERA-TEST, scale 1, no windows open)
Items: Apps button | Downloads, Trash (pinned is empty; the divider before Downloads).
- Glass (icon 48, gap 9, pad 12, bar 72, bottom gap 6, magnification ×1.6): pill x 857..1063, y 1002..1074; icon centres x Apps 894, Downloads 970, Trash 1027; y 1038. Hover band y 961..1074.
- Material (icon 48, pad 8, gap 9, bar 64, bottom gap 16): pill x 861..1059, y 1000..1064; icon centres x 894, 970, 1027; y 1032.
Each running app adds 57 px (icon + gap) before the divider and shifts the pill left by 28.5 px: re-measure from a shot.
Use `P glide` (not jumps) so hover enters/leaves. Record with wf-recorder at 60 fps; time with tools/anim-timing.py; widget reloads with tools/region-changes.py.

1. **Rows in place (the fixed bug)**: start with no windows. Run `foot &` (an app icon appears left of the divider, with its running dot), wait 2 s, then `foot &` again (no new icon), then close one foot window, then the other. Record 4 s around each close. Must see: on the first foot, only its icon inserted (Apps keeps its place; Downloads/Trash shift right as one block, they don't flash or reload); on closing the last foot window its icon goes and **no other icon hops** (QML: nothing bounces on close). Compare against the QML pass.
2. Bounce isn't handed to the wrong icon: click the foot icon when NOT running (pin foot first: set `dock.pinned` ["foot","firefox"]): foot hops twice (Glass 1040 ms total, Material 960 ms) and only foot. Then close foot: firefox must not hop.
3. Auto-hide (set `dock.autoHide` "always"): pointer at (1500,500); dock slides out (Glass 250 ms InCubic; Material spring ~320 ms). Glide to (960,1079) and rest: dock appears after 120 ms (Glass slide up 350 ms with a small overshoot, OutBack 1.2). Glide up to (960,700): it hides 300 ms after leaving the zone. Record and time with anim-timing; must match QML ±1 frame.
4. Auto-hide "windows": open foot maximized (`hyprctl dispatch 'hl.dsp.window.fullscreen(...)'` or a large foot) covering the bottom: dock hides; close it: dock shows.
5. Opening any panel closes the dock menu: right-click Trash (1027,1038 Glass / 1032 Material) → menu; then `ipc panels open controlCenter TESSERA-TEST` → menu gone immediately.

### Touchscreen (needs a touch device in the arena: e.g. a uinput touchscreen made with `evemu`/`python-evdev` mapped to TESSERA-TEST; skip with a note if the arena has none)
6. `dock.autoHide` "always", the dock hidden. Touch down at (960,1079), move up to (960,1040) over 150 ms, lift. Must see: the dock slides up (Glass 350 ms OutBack / Material spring) as soon as the finger passed 24 px, stays up ~4 s with no pointer near, then hides (250 ms). A touch moving only 20 px up: nothing.
7. Same, but glide the mouse pointer into the zone (960,1040) during the 4 s and keep it there: the dock stays up after 4 s.
8. Autohide "off" (core's touch): tap on Downloads (970,1038): the stack opens **once** (not opened and closed again); tap a file cell: it opens; tap outside the card: the stack closes. Tap Trash: one file manager window, not two. (A finger held on an icon may magnify it until it lifts: core moves the pointer with touches; QML doesn't.)
9. Right-click an icon with the mouse, then tap a menu row with the finger: the row's action runs once. A 500 ms hold on an icon (core: right click) opens its menu.


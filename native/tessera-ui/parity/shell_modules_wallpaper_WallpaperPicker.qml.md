# shell/modules/wallpaper/WallpaperPicker.qml → surfaces/wallpaper/src/wallpaper (+ ui/wallpaper.slint)

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Panel above the dock, overlay layer, width min(screen-80, 1200)+2m, height thumbH+78+2m, bottom margin over the dock | 66-72 | `panel_width`, `HEIGHT`, `bottom_margin` (tests hold them) | same |
| Keyboard exclusive while shown, none while closing; input only while shown | 64-68 | `open`/`close` set keyboard interactivity; close commits None | same |
| Open/close: progress 0↔1 over Theme.motion.panel (320), emphasized in / exit out (each look's curve), body opacity = progress, y + (1-progress)·16 | 44-52, 92-95 | `animate progress 320ms` with the look's beziers; body `y`/`opacity` | same |
| Window lingers then unloads after close (PanelLoader 600 ms) | PanelLoader | dropped when the close animation ends (`closed`) | same at a glance (QML keeps it 600 ms; not kept loaded either way) |
| Lists the wallpaper folder (newest first, its name filters) then the bundled ones (*.jpg by name), afresh at every open | 74-87 | `files::list` on a thread at every open (tests) | same |
| The thumbnails list | 166 ListView model | **Fixed**: was a new VecModel per listing handed to the window; now one `Rows<Thumb>` handed once, `set` per listing, thumbnails set in place, cleared on close | different → fixed |
| Current index = the applied picture at open | 47-48, 72 | `listed` → `set_current(applied)`, `reveal()` centres it | same |
| Glass panel radius 26, Material Theme.radius.panel (28); shadow; Material tonal surface (see-through per transparency) | 97-103 | body radius Theme.radius-panel (26/28), glass shape 26 via the plugin, shadow, `panel-alpha` | same |
| Title "Wallpaper" + folder (~) in tertiary small | 113-124 | title / folder Text | same (baseline vs bottom alignment: same at a glance) |
| "Open folder" button: Glass fill / fillHover; Material secondary container + state layers 0.08 hover / 0.1 press; closes the panel and opens the folder | 125-150 | `btn` + `bt` TouchArea; `open_folder` → close + `wallpaper.openFolder` | same |
| Esc closes, ←/→ move, Enter applies | 107-111 | `keys` FocusScope → dismissed / move / apply | same |
| Highlight moves (keys) over Theme.motion.popover, kept in the middle range (ApplyRange) | 172-175 | `move`: glide 220 ms to `centred(current)` | same |
| Thumbnail: 220×124, gap 12; Glass radius 12; Material 16, chosen 28 | 177-192 | `frame` | same |
| Material radius change on the MaterialSpring "default" (spring 6.08, damping 0.5) | 193 | was `400ms spring(0.6)` (a bounce the QML doesn't have). **Fixed**: Qt's SpringAnimation (16 ms steps, v += 6.08·Δ − 0.5·v) is critically damped at ≈26 rad/s → `242ms spring(0)` (simulated: within 0.03 of Qt's curve at every step) | different → fixed |
| Border: chosen 3 px accent; keyboard current 2 px (Material secondary, Glass label) | 194-195 | same expressions | same |
| Hover scale 1.03 over Theme.motion.hover | 196-197 | was a fixed 150 ms. **Fixed**: `Theme.hover` (follows the theme's pace when core sets it) | same value → follows Theme |
| Picture fades in when decoded over Theme.motion.popover | 207-208 | was a fixed 220 ms. **Fixed**: `Theme.popover` | same value → follows Theme |
| Material: 6 px gap inside the chosen one's outline | 201-203 | `inner.inset` | same |
| Click: current = it, apply (Wallpaper.set), pointing-hand cursor | 210-219 | `pick(i)` → `apply` → `wallpaper.set`; `mouse-cursor: pointer` | same |
| Scrolling: Momentum (touchpad glides on after the fingers lift), drag and flick (Flickable) | 164 | wheel + 50 ms pause → `fling`; drag/flick at 1500 px/s² | same, except: Slint gets no scroll phases, so a fast mouse wheel glides a little too; a flick stops at the ends (no overshoot) |
| Thumbnails decoded off thread, only in view (+ either side); far ones let go | 204-206 (asynchronous) | `thumbs`/`wanted`, Pool of 2 | same (better memory) |
| Empty: "No images in <folder>" | 224-230 | `listed && thumbs.length == 0` | same |
| Glass: on the plugin's glass, shape follows the open animation | 97-103 | `after_frame` sends the shape when it changes | same |
| surface_closed scoped | — | `Rc::ptr_eq` | same |
| Motion pace (snappy 0.85, cool-down) | Theme.motion.pace | panel 320 ms fixed; hover/popover follow Theme tokens (core doesn't set a pace yet) | different: needs core to set `Theme.hover/popover` (and a panel duration token) from the pace |

Icons: the picker has no icon-theme icons (no Colloid lookup); thumbnails are the pictures themselves.

## Test plan (Sonnet tester, arena 5, TESSERA-TEST 1920x1080 scale 1, user's config: Material dark, expressive; then Glass; QML and Rust)

The panel is centred above the dock: 1248 wide (x 336..1584 incl. 24 px shadow room), thumbnails 220x124 with 12 px gaps, the strip about y 850..982 with a 48 px dock (find the exact row in the first shot).

1. **Open.** Pointer at 1500,400. Record 1.0 s while `ipc panels open wallpaper TESSERA-TEST`. anim-timing on the panel region (336,780 1248x250): opacity 0→1 and a 16 px rise over **320 ms**, Material emphasized-decelerate (0.05,0.7,0.1,1) / Glass (0.2,0.9,0.1,1). Thumbnails fade in over **220 ms** as they decode. The applied picture has a 3 px accent ring (Material: rounder, radius 28, with a 6 px gap inside) and sits in the middle if the list is long enough.
2. **Hover.** Glide (`pointer glide X Y 400`) onto the second visible thumbnail centre: it scales to **1.03 over 150 ms**; glide off: back over 150 ms. Pointer cursor is a hand.
3. **Keys.** Press Right twice: a 2 px ring (Material: secondary colour; Glass: label colour) moves one thumbnail per press; when it nears the edge the strip glides **220 ms** to keep it in the middle. Left goes back.
4. **Apply (keys).** Press Return: the background crossfades (700 ms, see Background plan), the ring jumps to it. **Material**: the newly chosen thumbnail's corner rounds 16→28 critically damped (no overshoot) in about 250-300 ms to settle, the old one 28→16 the same way. Record 1.6 s and compare the radius curve with the QML pass (MaterialSpring default).
5. **Apply (click).** Click another thumbnail: it becomes chosen and the strip glides it to the middle (220 ms).
6. **Scroll.** Pointer over the strip at 900,915: `pointer swipe 300 120` (touchpad): the strip scrolls and keeps gliding briefly after the fingers lift, decelerating; `pointer scroll 3` (wheel): steps of 45 px. Drag with the left button from 1200,915 to 800,915 over 300 ms and release: it follows the pointer, then flicks on; no thumbnail is applied by the drag.
7. **Open folder.** Hover the "Open folder" pill (top right of the panel): Material state layer 8% (pressed 10%), Glass fill→fillHover. Click: the panel closes and the file manager opens the wallpaper folder.
8. **Close.** Reopen; press Esc: the panel fades and sinks 16 px over **320 ms** with the exit curve (Material 0.3,0,0.8,0.15 / Glass 0.4,0,1,1); clicks pass through during the fade.
9. **Reopen** twice in a row: the list comes up each time with thumbnails (no stale or blank tiles), the applied one ringed.
10. **Empty folder:** point `wallpaper.directory` at an empty folder with no bundled pictures available: "No images in <folder>" centred in the strip.
11. Repeat 1-8 in Glass: the panel is on the plugin's glass, radius 26, thumbnails radius 12, no radius animation.

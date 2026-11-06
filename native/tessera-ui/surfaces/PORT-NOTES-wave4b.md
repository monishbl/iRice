# Wave 4b: the QML windows that had no Rust version

Written, not compiled or run (standing order). Everything below draws only in
`tessera-ui --standalone` (`tessera_core::host::standalone()`), except the
Downloads stack, which belongs to the Rust dock and so works in both modes.
With the QML shell running, its own windows are still there and a second set
would show twice; none of the QML twins got a `RustUi.drawing(...)` guard.

## Wiring (shared files)

- `native/tessera-ui/Cargo.toml`: `tessera-screenfx`.
- `native/tessera-ui/src/surfaces.rs`: KNOWN and `make()` gained
  `downloadsstack` (dock crate), `widgetOptions` (desktop crate),
  `screencorners`, `findpointer`, `spaceswitch`, `displayconnect`,
  `displayoverlay`, `shelfprobes` (new crate `surfaces/screenfx`).
- host/system: `src/input/displays_feed.rs` (new) + `pub mod` and one line in
  `services()` of `src/input/mod.rs`.
- host/desktop: `src/wm/spaceswitch.rs` (new) + `mod spaceswitch;` and two
  one-line calls in `src/wm/mod.rs` (`Wm::event`, `Wm::config_changed`).
- Existing surface crates edited: `surfaces/desktop` (gallery.rs, options.rs,
  ui/options.slint, the Gallery component appended to ui/desktop.slint, small
  hooks in lib.rs, `widget()` split into `widget()`/`widget_of()`),
  `surfaces/dock` (stack.rs, ui/stack.slint, hooks in lib.rs).

## Things for the lead

1. `tests/test_rust_ui.py` will complain, for reasons that are by design:
   - `known_surfaces()` reads names with `[a-z]+`; `widgetOptions` has a
     capital (the host's Panels opens the panel `widgetOptions`, and
     `surface_of()` has no entry for it, so the surface has that exact name).
     Alternative: name it `widgetoptions` and add `("widgetoptions", "widgetOptions")`
     to `PANEL_NAMES` in host/shell/src/panels.rs (not mine to edit).
   - the 8 new names have no QML twin that checks `RustUi.drawing(...)`
     (standalone only), and a few `shell::ask` calls (`displays.keep|revert|finish`,
     `panels.set`) are answered by the host, not by RustUi.qml. The test needs
     an allow-list of "standalone only" surfaces.
2. No CORE-NEEDS. Everything went into the two host crates by new files plus the
   registration lines above.
3. Verification states to add (see each piece's "check").

## screenfx (surfaces/screenfx)

One crate, six parts. Slint has no custom shaders, so effects are drawn with
rectangles, gradients and clipped pictures and animated from Rust timers
(16 ms) with the QML's curves.

### ScreenCorners (`screencorners`) ... ScreenCorners.qml
- Ported: `appearance.screenCorners` (default on) and `screenCornerRadius`
  (12): a strip of that height along the bottom of every screen, overlay layer
  (`tessera:corners`), a black concave corner at each end, no input. Rebuilt
  when the setting changes.
- Differs: nothing known.
- Check: bottom corners of every display are rounded; Settings → Appearance
  corner switch/radius updates them live. No ipc.

### FindPointer ring (`findpointer`) ... services/FindPointer.qml
- Ported: the ring (150 px, accent 12% fill, 3 px line at any scale; scale
  1.6 → 0.5 in 450 ms ease-out cubic, opacity in 200 ms, hold, out in 300 ms
  ease-in), on the display under the pointer (else the focused one), following
  the pointer, not drawn until the position is known, click-through. Fed by
  host/desktop `wm/findptr.rs` (`{"to":"findpointer","show":true|false,x,y}`),
  which already existed.
- Differs: when the pointer crosses to another display the window is remade and
  the animation restarts (same as the QML's LazyLoader).
- Check: shake the pointer (Settings → Mouse & Trackpad → Shake to find the
  pointer, ring or both), or `tessera-ui ipc call findpointer find`.

### SpaceSwitch (`spaceswitch`) ... osd/SpaceSwitch.qml
- Ported: glow (four edge gradients, 72 px deep, accent light shade; Material
  primaryFixedDim) or dim (18% scrim) on the display you moved to; timings 150
  ms in / 250 held / 450 out (ease-out, ease-in); reduce motion 100/300, the dim a
  plain linear 100/150/250. The decision is host/desktop `wm/spaceswitch.rs`
  (focusedmonv2, 2+ displays, `workspaces.switchCue` glow|dim|off with the
  older `dimOthers`, not within 4 s of a hotplug (`displayfx.hotplugAt` state
  from the displays feed), not while Spaces is quiet or the Overview / Mission
  Control is open). That file also carries the other half of SpaceSwitch.qml:
  `cursor.no_warps` from `workspaces.warpPointer` (on change, and after every
  `configreloaded`).
- Differs: the accent "ramp[3]" is approximated by the accent mixed 55% toward
  white.
- Check: with two displays, move focus to the other (Super+arrow, or the
  pointer crossing). Without a second display:
  `tessera-ui ipc call cues space [DP-1] [glow|dim]`.

### DisplayConnect (`displayconnect`) ... osd/DisplayConnect.qml + displayfx.frag
- Ported: per-screen overlay while a run plays; `lead`, durations 1100/1050/950
  (arrived) and 1000/800 (beside) × Cool-down's motionScale, the QML's easings,
  220 ms fade, 250 ms wait for the picture, 3 s limit for a pending run,
  reduced motion (fade from dark; neighbours nothing), `displays.finish [id]`
  at the end. Wallpaper decoded off the loop, cropped like the desktop's. Fed by
  host/system `input/displays_feed.rs` (`{"to":"displayconnect","runs","motion"}`)
  from the bus entry `displayfx` that displays.rs already kept.
- Differs: the three looks are approximations of the shader: extend = a soft
  wipe without motion blur or sliding picture; liquid = a clipped circle (no
  feathered edge, no neck/gooey blend) and, beside, a capsule and a drop with
  a rim (no refraction); warp = the wipe with a ring of light at its front (no
  lens); beside-warp = a ring band plus an edge glow. The hand-over to the
  new display's spaces happens at the end of the effect (displays.rs
  `finish`), not as the fade starts.
- Check: `tessera-ui ipc call displays preview extend|liquid|warp left|right|top|bottom`
  (existing, host/system), or plug/unplug a display (Settings → Displays →
  When a display connects).

### DisplayOverlay (`displayoverlay`) ... DisplayOverlay.qml, MaterialDisplayIdentify.qml, MaterialDisplayConfirm.qml
- Ported: Identify (top left of each real screen, margins 32/48: number 72 px,
  name, "DP-1 · 2560 × 1440"; Glass on the plugin's glass, Material a cookie in
  primaryContainer on a card that springs in) and Confirm ("Keep these display
  settings?", the countdown in words, Go back / Keep settings; Material adds
  the wavy bar running down). HEADLESS-* / TESSERA-* screens get nothing.
  "Go back" → `displays.revert`, "Keep settings" → `displays.keep`.
- **The revert is the host's, as in the QML** (`Displays.qml` confirming): the
  15 s countdown and the `revert()` run in host/system displays.rs
  (`M::Countdown`) whether or not a card exists on any screen; this surface only
  shows `confirmLeft` and answers. If nobody answers, the host reverts at 0.
  The cards take the keyboard on demand (Tab / Space / Enter in Material).
- Differs: cookie9 is the polar outline, not the shader shape; the Confirm
  window is a fixed size (the Material card centred in it, input region = the
  card).
- Check: Settings → Displays → Identify; change a resolution/scale/rotation
  (Keep? appears on every screen, the countdown ticks, Go back reverts, doing
  nothing reverts at 15 s). Without touching settings:
  `tessera-ui ipc call cues identify` and `tessera-ui ipc call cues confirm`
  (a 6 s preview; "Go back" then changes nothing since nothing is pending).

### ShelfProbes (`shelfprobes`) ... shelf/ShelfProbes.qml
- Ported: while the host's `shelf.probing` is true (a held button): 3 px strips
  at each screen's left, right and top (when `shelf.edge`, default on) and a
  360 px square under the pointer after a shake (`shelf.probe` {x,y,screen}),
  all taking drops. A drag entering one → `shelf` ask `dragArrived [where, screen, y]`;
  leaving → `dragLeft [where]`; a drop → `offer [screen]` then `keep [uris, text]`.
- Differs: the drag's URIs are only known at the drop, so a drag that offers a
  `text/uri-list` opens the shelf (a browser link drag offers one too); the
  drop keeps only if some URI is `file:`, otherwise refuses it.
- Check: drag a file from a file manager to the left/right/top edge of a
  screen and hold ~0.35 s (the shelf opens); or drag a file while shaking the
  pointer sideways (the shelf opens on that side). Needs the button held, so no ipc.

## desktop crate (surfaces/desktop)

### WidgetGallery ... desktop/WidgetGallery.qml (gallery.rs, ui/desktop.slint `Gallery`)
- Lives in the Desktop part (its previews are the desktop's own widgets,
  built by `widget_of()` with the desktop's live data). Up while Edit Widgets is
  on (the desktop feed's `editing`, on `edit_screen` else the first screen),
  standalone only. Glass sheet along the bottom, 96 px above the dock (Layer::Top,
  keyboard on demand), kinds list on the left (click, Up/Down), name and
  description and Done on the right, each size as the widget scaled to 56% with
  the add badge, hover border, 0.95/1.04 scale. A pick calls the desktop's own
  `add()` directly (first free cell; "No room" via the island as before). Done
  or Escape: `desktop endEdit` + `panels.set widgetEditing false`. Material:
  the same layout with the secondary-container list, tonal size chips and a
  primary Done.
- Differs: previews show live data, not the QML's sample data; no momentum
  scrolling or elastic press; no corner clipping of a preview's content.
- Check: right-click the desktop → Edit Widgets (or Settings' Edit Widgets).

### WidgetOptions (`widgetOptions`) ... desktop/WidgetOptions.qml (options.rs, ui/options.slint)
- Own part, named like the panel. A centred dialog (420 px content, scrim, spring
  in, 192 ms out, Esc / click outside / Enter / Done close it): photos folder
  (field + folder chooser via shell/assets/scripts/pick-file.py; the dialog
  closes for it), world-clock cities, stock symbols (saved when the field loses
  focus or on closing: `desktop.photosFolder|worldClocks|stocks`), Smart Rotate
  switch (`desktop.smartRotate`), graphs Area/Bars/Line
  (`resources.graphStyle`). The kind comes from the shell state
  `panels.widgetOptionsKind` (it can arrive just after the command to open; the
  dialog fills when it does).
- Check: right-click a Photos / World Clock / Stocks / Widget Stack / System
  widget → Widget Options….

## dock crate (surfaces/dock)

### Downloads stack, Glass and Material (`downloadsstack`) ... dock/DownloadsStack.qml, MaterialDownloadsStack.qml, DownloadsFiles.qml
- Found by the sweep (the task named only the Material one): the Rust dock
  opened the folder; the stack was QML-only. Now a click on the Downloads item or
  "Show Recent Files" in its menu opens it: up to 12 newest files of the
  downloads folder, four to a row, above the dock. Click opens (xdg-open),
  right-click shares (`panels.share`), the eye is Quick Look
  (`panels.quickLook`), "Open in Files"; Esc / click outside / another panel
  opening closes it. The dock stays out meanwhile (`popup_open`). Glass on the
  plugin's glass (scaled about the dock); Material: title, text button, state
  layers, the empty state with the cookie.
- Differs: names are shortened in the middle in Rust; the folder is listed once as
  it opens.
- Check: click the Downloads item in the dock (works with the QML shell too,
  since the Rust dock opens it).

## Sweep: other QML windows with no Rust twin

Method: every QML file with PanelWindow / FloatingWindow / PopupWindow /
WlSessionLock / GlassWindow / LazyLoader, its parent chain up to
TesseraShell.qml, checked against `RustUi.drawing(...)` gates and the Rust
surfaces. Found missing: the nine above plus the Glass Downloads stack. Checked and
covered by a Rust surface: everything else under modules/ (menubar incl.
NowPlaying / Menus / StatusMenus / BarMenu / HoverTip / IslandAlertStack,
stage strip, app library, decorations, captions, tabs, snap preview, quick
look, share cards, shelf window and drop zone, screen tools, lock and login
intro, session / About, polkit, welcome, settings incl. its fingerprint sheet,
greeter). Not ported on purpose: assets/lookswitch/shell.qml and
assets/updater/shell.qml are separate Quickshell configs (own `qs -p` process
that must outlive the shell while it restarts), and dev/GlassLab.qml is a dev tool.

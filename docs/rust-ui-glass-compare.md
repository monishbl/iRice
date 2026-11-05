# Glass look: the QML surfaces against their Rust twins

Measured 5 November 2026 in sandbox arena 7, main at `b8adf2d` (tessera-ui release build, every surface on main in Rust: menubar, dock, osd, emoji, clipboard, background, wallpaper, today, desktop). Method and tools as in [rust-ui-spike-slint.md](rust-ui-spike-slint.md); porting guide in [rust-ui-surfaces.md](rust-ui-surfaces.md).

## In short

- **The real glass plugin works for both.** The repo's `tessera-glass` built in the container against its Hyprland 0.56.2 headers and loaded by the arena's config: `hyprctl tessera-glass ping` answers `tessera-glass 5`, the QML shell logs `Glass sync: native (Tessera.Native)` (the native helpers built too), tessera-ui logs `glass plugin found`.
- **The biggest visual difference is a bug, not Slint:** tessera-ui ignores the glass settings. `theme::glass_shape` hard-codes the Liquid preset (lens 1, frost 0.75, tint 0.75, IOR 1.5), so at the settings measured here (frost 25%, tint 100%, IOR 1.33) every Rust panel is three times frostier, darker, glossless and bends more than its QML twin. It shows on every menu and panel (mean difference 18 to 42 of 255 inside menus and panels; 1 to 3 where there is no glass).
- **CPU per interaction: Rust is about 1.5 to 2 times cheaper for the bar, island, dock, OSD, emoji and clipboard** (the QML shell plus Hyprland against tessera-ui plus the QML shell plus Hyprland). **It is dearer for the full-screen surfaces**: Today 599 against 360 ms per open and close, background 512 against 368 ms per wallpaper change, desktop 257 against 151 ms per menu, the wallpaper picker 437 against 187 ms. Most of that is Hyprland's own CPU (up to 3.4 times the QML case), which points at full-surface damage on large surfaces and extra full-screen layers.
- **Memory goes up, not down, for now:** with all nine surfaces in Rust the QML shell stays at 387 MB resident (408 MB with everything in QML), and tessera-ui adds 135 MB: 521 MB together, +28%. The QML shell keeps its services and singletons loaded when its surfaces step aside.
- **Start-up of tessera-ui:** first frame 107 ms after exec, the bar mapped at 36 ms, the wallpaper at 167 ms (medians of 3). Idle CPU is at the floor for all processes (0.1 to 0.2% of a core).
- **Frame pacing** (the plugin's `frametimes` on IRICE-TEST) is the same or better in Rust for 8 of 11 interactions; worse for Today (5.5% of frames late against 0.4%).

## Setup

| | |
|---|---|
| Arena | 7 (hidden, silent: KWin virtual display, private PipeWire), IRICE-TEST 1920×1080 at scale 1, WAYLAND-1 disabled |
| Compositor | Hyprland 0.56.2 in the `irice` container, the repo's `hypr/plugin/tessera-glass` built with `make` against it |
| Look | Glass, dark, accent blue, motion subtle, wallpaper `site/film/tessera/capture/seed/photos/wall-tiles.jpg` for every shot |
| Glass | `appearance.glass: "custom"`, `glassLens: 1` (the "100% glass" knob, at its maximum; `glassSolid: 0`), `glassFrost: 0.25`, `glassTint: 1`, `glassIor: 1.33`; refraction band 1, dispersion 0.35, depth auto (defaults) |
| Other | dock magnification on at 1.45 (the default; this arena's seed had 1.15), island HUD on (the OSD capsule states turn it off), desktop widgets off except for the desktop states and the memory runs (then Config.qml's default four widgets and the desktop files) |
| QML side | `rust.surfaces: []`, the shell's native helpers (`shell/native/build.sh`) built for the container's Qt 6.12 |
| Rust side | `rust.surfaces` = all nine, release build at `b8adf2d`, installed in the arena's data dir behind a wrapper that keeps its stderr |
| Host | `performance` power profile, on AC. Load and package temperature were recorded for every measurement (below) |

Every CPU and frame measurement ran under the lead's `quiet.sh` (all build slots held, so no agent compiled meanwhile), in holds of about two minutes, released between.

Measurement scripts (session scratchpad, not committed): `a7/perf.py` (CPU per cycle from `/proc/PID/stat` of the QML shell, tessera-ui and the nested Hyprland; frame pacing from `hyprctl tessera-glass frametimes`, reset before each test; memory from `/proc/PID/status` and `smaps_rollup`; tessera-ui start-up by polling Hyprland's layer list every 2 ms), `a7/shots.sh` (every state, `grim` on IRICE-TEST), `a7/compare.py` (pairs, differences, 2× crops), `a7/flick.sh` and `a7/cold.sh` (60 fps recordings of opening, `a7/frames.py` finds the onset, flicker and settling).

## Performance

### CPU per interaction

Ten cycles per test after one warm-up cycle, three rounds; the median of the three rounds' per-cycle averages. ms of CPU per cycle (user + system).

| Interaction (one cycle) | QML: shell | QML: Hyprland | **QML total** | Rust: tessera-ui | Rust: QML shell | Rust: Hyprland | **Rust total** | Rust / QML |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Menu bar: system menu open (click), Esc | 197 | 94 | **291** | 89 | 7 | 104 | **201** | 0.69 |
| Island: volume change, HUD shows and closes | 222 | 82 | **311** | 89 | 9 | 87 | **182** | 0.59 |
| Dock: pointer glides across (magnify) and away | 129 | 66 | **196** | 33 | 3 | 79 | **113** | 0.58 |
| OSD capsule: volume change (island HUD off) | 78 | 33 | **110** | 33 | 6 | 37 | **80** | 0.73 |
| OSD capsule: brightness preview | 75 | 34 | **109** | 30 | 4 | 40 | **74** | 0.68 |
| Emoji picker: open, close | 363 | 70 | **433** | 121 | 8 | 81 | **216** | 0.50 |
| Clipboard: open, close | 244 | 61 | **305** | 101 | 9 | 105 | **215** | 0.70 |
| Wallpaper picker: open, close | 144 | 43 | **187** | 289 | 8 | 124 | **437** | 2.34 |
| Today: open (bloom), close | 264 | 96 | **360** | 249 | 17 | 326 | **599** | 1.66 |
| Background: wallpaper changed (config) | 305 | 63 | **368** | 231 | 123 | 158 | **512** | 1.39 |
| Desktop: right-click menu, Esc | 96 | 55 | **151** | 71 | 8 | 178 | **257** | 1.70 |

Per-round totals (QML / Rust), to show the spread: menu bar 291, 317, 286 / 235, 182, 201; emoji 433, 360, 552 / 216, 210, 244; Today 393, 241, 360 / 436, 599, 648; background 398, 265, 368 / 426, 512, 621; wallpaper picker 187, 142, 262 / 437, 418, 467. The direction holds in every round.

Notes:
- With the background in Rust the QML shell still spends 123 ms per wallpaper change (its Wallpaper service, colours and the QML wallpaper window, which stays mapped; see issues).
- Hyprland's share grows for every Rust surface that covers a large area: Today ×3.4, desktop ×3.2, wallpaper picker ×2.9, background ×2.5, clipboard ×1.7. tessera-ui (FemtoVG) redraws and damages whole surfaces; the desktop and the background are full-screen layers, and in Rust mode there are four full-screen layers on the bottom levels instead of one (see issues).

### Frame pacing (compositor side)

The plugin's `frametimes` for IRICE-TEST over each test's ten cycles: frames on time / one vsync missed / two missed / longer (gaps over 250 ms between animations are left out by the plugin), worst interval, and the share of frames late. Medians of three rounds.

| Interaction | QML | Rust |
|---|---|---|
| Menu bar: system menu | 665 / 2 / 9 / 2, worst 122 ms, 2.2% late | 780 / 0 / 10 / 10, worst 216 ms, 2.5% late |
| Island: volume HUD | 940 / 2 / 0 / 1, worst 142 ms, 0.5% | 777 / 0 / 0 / 1, worst 218 ms, 0.1% |
| Dock: magnify | 558 / 12 / 10 / 12, worst 163 ms, 5.7% | 558 / 14 / 10 / 12, worst 184 ms, 6.2% |
| OSD: volume | 291 / 0 / 0 / 2, worst 202 ms, 1.0% | 292 / 1 / 1 / 3, worst 221 ms, 1.7% |
| OSD: brightness | 290 / 0 / 0 / 2, worst 207 ms, 0.7% | 297 / 2 / 0 / 3, worst 243 ms, 1.3% |
| Emoji: open, close | 501 / 5 / 7 / 49, worst 200 ms, 11.1% | 426 / 1 / 1 / 5, worst 246 ms, 1.6% |
| Clipboard: open, close | 500 / 0 / 1 / 10, worst 247 ms, 2.2% | 494 / 0 / 0 / 5, worst 249 ms, 1.4% |
| Wallpaper picker | 419 / 0 / 0 / 3, worst 198 ms, 0.7% | 433 / 0 / 0 / 0, worst 39 ms, 0.2% |
| Today | 723 / 1 / 0 / 2, worst 91 ms, 0.4% | 671 / 18 / 8 / 2, worst 123 ms, **5.5%** |
| Background: wallpaper change | 411 / 9 / 7 / 3, worst 90 ms, 3.4% | 443 / 3 / 0 / 0, worst 31 ms, 1.2% |
| Desktop: menu | 417 / 10 / 10 / 13, worst 136 ms, 7.3% | 572 / 20 / 11 / 12, worst 237 ms, 7.0% |

The QML emoji picker drops the most frames (49 long stalls in ten opens); in Rust it's smooth. Today in Rust misses one or two vsyncs on 26 of 697 frames: the full-screen surface's cost in Hyprland, above.

### Memory and idle CPU

After a fresh shell start (`./dev.sh reload`) and 60 s, with the default desktop widgets and files showing; then idle CPU over the next 60 s. Three runs each; medians. No swap was in use during these runs (an earlier set, taken while other agents' builds pushed 12 GB into swap, was dropped: resident memory then said nothing).

| | Everything in QML | All nine surfaces in Rust: QML shell | tessera-ui | Together |
|---|---:|---:|---:|---:|
| Resident (VmRSS) | 408.4 MB (395.9, 408.4, 412.1) | 386.5 MB (381.1, 386.5, 394.3) | 134.7 MB (121.0, 134.7, 135.8) | **521.2 MB (+28%)** |
| Anonymous (RssAnon) | 261.1 MB | 260.1 MB | 76.8 MB | **336.9 MB (+29%)** |
| PSS (shared libraries counted once) | 357.7 MB | 341.1 MB | 112.7 MB | **453.8 MB (+27%)** |
| Threads | 49 | 35 | 37 | 72 |
| Idle CPU over 60 s (% of one core) | shell 0.15, Hyprland 0.12 | shell 0.18 | 0.20 | Hyprland 0.22 |

tessera-ui right after start (three start-ups, widgets off): 117 to 118 MB resident, 59 to 60 MB anonymous, 36 to 37 threads.

### tessera-ui start-up

`rust.surfaces` set from empty to all nine in the running shell (the shell starts tessera-ui); the time from tessera-ui's exec (first seen by a 2 ms `/proc` poll) to each of its layers appearing in Hyprland's layer list, and to its own `first-frame` log line.

| | run 1 | run 2 | run 3 | median |
|---|---:|---:|---:|---:|
| Config written → exec | 14.5 ms | 19.6 ms | 12.8 ms | 14.5 ms |
| Exec → bar layer (`tessera:bar`) | 111.7 ms | 35.8 ms | 33.1 ms | 35.8 ms |
| Exec → dock layer (`tessera:dock`) | (the QML dock's, same name) | 56.1 ms | 49.2 ms | 52.7 ms |
| Exec → background layer (`tessera:background`) | 247.2 ms | 167.2 ms | 163.9 ms | 167.2 ms |
| Exec → first frame drawn | 190.1 ms | 107.0 ms | 94.0 ms | 107.0 ms |
| Glass plugin found | yes | yes | yes | |

(Run 1 matched the dock layer by name only, and the QML dock uses the same namespace; runs 2 and 3 match by PID.)

### Host during the measurements

Load average (1 min) and package temperature at the start of each hold. QML rounds: 17:26 to 18:06, load 1.0 to 4.3, 52 to 99 °C (the end of other agents' builds; the hold itself had no build running). Rust rounds: 18:20 to 18:36, load 0.3 to 1.7, 61 to 73 °C. Memory runs: QML 18:10 to 18:15, load 1.2 to 1.9, 67 to 78 °C; Rust 18:40 to 18:45, load 0.8 to 1.5, 66 to 69 °C. The QML rounds ran warmer, which can only flatter Rust where Rust wins; where Rust loses (Today, background, desktop, wallpaper picker) it lost on a cooler machine.

## Visual differences

Screenshots in `native/tessera-ui/review/glass-compare/` (not committed): `<surface>-<state>-qml.png` / `-rust.png` (the whole screen), `-pair.png` (the surface's region side by side), `-diff.png` (|QML − Rust| ×4), `-edge2x.png` (a glass edge at 2×). Mean difference inside each region (0 to 255) and the share of pixels more than 24 off:

| State | Region (x0, y0, x1, y1) | Mean diff | Pixels > 24 off |
|---|---|---:|---:|
| menubar-idle | 0, 0, 1920, 40 | 2.4 | 2.2% |
| menubar-system | 0, 0, 380, 360 | 23.8 | 41.6% |
| menubar-window | 90, 0, 450, 720 | 42.5 | 69.4% |
| menubar-status (the QML popover in both) | 1500, 0, 1920, 300 | 9.0 | 12.5% |
| menubar-island-volume | 700, 0, 1220, 90 | 3.2 | 0.7% |
| dock-idle | 600, 940, 1320, 1080 | 4.1 | 1.3% |
| dock-magnified | 600, 860, 1320, 1080 | 3.3 | 1.4% |
| dock-menu | 600, 780, 1320, 1080 | 8.1 | 11.5% |
| osd-volume | 1560, 0, 1920, 120 | 7.1 | 4.7% |
| osd-brightness | 1560, 0, 1920, 120 | 7.6 | 5.2% |
| emoji-open | 700, 290, 1220, 790 | 32.6 | 61.6% |
| clipboard-open | 630, 200, 1300, 790 | 18.2 | 46.1% |
| background-idle | whole screen | 0.9 | 0.2% |
| wallpaper-picker | 320, 760, 1600, 1080 | 18.8 | 24.7% |
| today-open | 480, 190, 1440, 880 | 1.7 | 1.2% |
| desktop-idle | whole screen | 5.1 | 8.1% |
| desktop-menu | 1300, 450, 1900, 950 | 7.4 | 11.7% |

(The clock, the battery level and the weather moved between the QML and the Rust shots, which were taken about an hour apart; those small areas are in the numbers.)

### The glass itself (every surface on glass)

tessera-ui's `theme::glass_shape` (src/theme.rs) uses fixed numbers, "the default Liquid material: lens 1, frost 0.75, tint 0.75", and never reads `appearance.glass`, `glassLens`, `glassFrost`, `glassSolid`, `glassTint`, `glassIor`, `glassRefraction`, `glassDispersion`, `glassDepth` or `wallpaperTint`. What each sends the plugin at these settings (dark), from Theme.qml's `glass.preset()` and `glass_shape`:

| Plugin parameter | QML, thin shape | Rust, thin | QML, thick (≤ 140 px) | Rust, thick |
|---|---:|---:|---:|---:|
| refraction | 16.4 | 22.0 | 22.3 | 30.0 |
| depth | 0 | 0 | 0.41 | 0.55 |
| bevel | 18 | min(18, radius) | 26 | min(26, radius) |
| frost | **0.25** | **0.75** | **0.25** | **0.75** |
| chroma (dispersion) | 0.18 | 0.18 | 0.28 | 0.28 |
| rim | 0.28 | 0.28 | 0.28 | 0.28 |
| tint alpha | 0.337 | 0.396 | 0.337 | 0.396 |
| tint colour | the base tinted 10% toward the wallpaper's tone | fixed grey | same | same |
| saturation | 1.39 | 1.46 | 1.39 | 1.46 |
| gloss | **0.79** | **0.36** | **0.79** | **0.36** |

So every Rust panel reads as frosted, darker grey glass where the QML one is clear glass that shows the tiles through it with a bright specular rim (`menubar-window-pair.png`, `emoji-open-pair.png`, `clipboard-open-pair.png`, `menubar-system-edge2x.png`). The Rust numbers equal QML's only at the default Liquid material with IOR 1.5 and no wallpaper tint. The shapes are in the right place: in every 2× crop the Rust glass corners and edges sit exactly on the drawn panels.

### Per surface

**Menu bar** (`menubar-*`)
- Idle bar: the same layout, text and spacing (2.4 mean). Status icons differ in data rather than drawing: Bluetooth shows as on in Rust and off (slashed) in QML; the battery glyph is a bolt (charging) in Rust and a plug (on AC) in QML. tessera-ui reads BlueZ and UPower itself; the QML shell's services report differently in the sandbox.
- System menu and Window menu: same rows, widths, separators, shortcuts and the Move & Resize grid; the difference is the glass (above). The Window menu is the largest difference measured (42.5).
- Status menu: the battery popover is the QML shell's in both (the Rust bar opens it), so it matches; but see issue 3 (the Rust bar's battery item stays highlighted after the popover closes).
- Island with the volume HUD: near identical (3.2); the HUD's black island is drawn by both the same way.

**Dock** (`dock-*`)
- The bar and its glass line up (4.1). Icons differ: the apps button is a 3×3 dot grid in Rust and the four-squares glyph in QML; the trash is a different glyph; Foot's icon keeps its frame in Rust and loses it in QML's Clear icon style. Magnification and the hover label match (`dock-magnified-pair.png`).
- Right-click menu: same items and place; while it's open the QML dock keeps the hovered icon magnified, the Rust dock drops the magnification (`dock-menu-pair.png`).

**OSD** (`osd-*`, island HUD off)
- The capsule's size, place, icon and chevron match; Rust's glass is darker. The brightness preview shows a different level (about 40% QML, 50% Rust): each reads the backlight itself.

**Emoji** (`emoji-open-*`)
- Layout, grid, sections, the search field and the key hints match. The selected category has a pill behind it in Rust (QML: the underline only). Glass as above.

**Clipboard** (`clipboard-open-*`)
- Layout, pinned row, thumbnails, filters and key hints match; glass as above.

**Background** (`background-idle-*`)
- The same picture, scaled and placed the same (0.9).

**Wallpaper picker** (`wallpaper-picker-*`)
- QML's panel is barely visible at these settings (clear glass, low tint): the "~/Pictures/Wallpapers" caption and the "Open folder" button lose contrast on a light wallpaper. Rust's panel is a visible frosted panel with a pill behind "Open folder". Same thumbnails, order and selection ring.

**Today** (`today-open-*`)
- Near identical (1.7): greeting, events, to-dos, weather and buttons; the backdrop blur matches.

**Desktop** (`desktop-*`)
- The widgets sit on the same grid and the files in the same column. Differences: the widgets' glass (darker, frostier in Rust); the Batteries widget shows one ring with "100%" in QML and two rings without a percentage in Rust (Rust also lists a connected Bluetooth device's battery); the small calendar's header reads "THU 5 NOV" in QML and "THURSDAY" in Rust; the desktop files' icons are themed in QML (a PDF and an archive icon) and generic documents in Rust.
- Right-click menu: same items; Rust draws separators between the groups, QML's don't show at these settings.

## Other issues, by how visible they are

1. **The glass settings are ignored in Rust** (above): every glass surface looks like the Liquid preset whatever is chosen in Settings → Appearance (Frosted, Solid, Custom, IOR, refraction band, dispersion, depth, wallpaper tint). `glass_shape` doesn't read Reduce Transparency or Cool-down's cheaper glass either (not checked whether other code covers them). The fix is a Rust twin of `Theme.glass.preset(minSide)` fed from config.json; `glass_shape`'s `lens` argument (menus 0.35) multiplies the lens amount, as `MaterialMenu`'s `lens` does.
2. **Full-screen Rust surfaces cost Hyprland 2.5 to 3.4 times more**, and Today drops frames (5.5% late). Contributing: tessera-ui redraws and damages a whole surface for any change, and in Rust mode four full-screen layers sit on the background and bottom levels (`tessera:background`, two `tessera:desktop` layers and the QML shell's own `tessera:wallpaper`, which stays mapped while the Rust background draws) where the QML shell has one.
3. **The Rust bar's battery item stays highlighted after its popover closes with Esc** (visible in every later shot: `menubar-island-volume-rust.png`, `dock-idle-rust.png`, `desktop-menu-rust.png`); the QML bar clears it. The bar doesn't hear that the QML popover closed.
4. **The first emoji open after tessera-ui starts is late and loses its animation**: three cold opens appeared 0.08, 0.32 and 0.77 s after the call, the two late ones in a single frame with no opening motion; the QML picker appears in 0.15 to 0.2 s and animates every time (`flick/cold-emoji-*.mp4` in the scratchpad). With the build before 6cc00f2 the first open even showed the empty glass for over a second.
5. **Clipboard opens glass first**: for about 4 frames (67 ms) the Rust panel's glass shows with nothing on it, then the rows fade in without their thumbnails, which pop in about 130 ms later. QML fades glass, rows and thumbnails in together.
6. **The QML shell doesn't shrink when its surfaces move**: 387 MB resident with all nine in Rust against 408 MB with none, so Rust mode costs 113 MB more in total today. The services and singletons the surfaces used stay loaded (the background's case shows in CPU too: 123 ms of QML shell CPU per wallpaper change with the background in Rust).
7. **The wallpaper picker costs twice the CPU in Rust** (289 ms of tessera-ui per open and close against 144 ms of QML), probably the thumbnails decoded again on every open.
8. **tessera-ui starts twice at every shell start**: its log shows a first frame, `bye`, and a second start within 3 to 5 s, each time the shell (re)loads with a non-empty `rust.surfaces` (RustUi.qml's arming sees the list change twice). The surfaces it already drew are torn down and drawn again.
9. **Closed Rust panels stay in Hyprland's layer list** (`tessera:barmenu`, `tessera:clipboard`, `tessera:contextmenu`, `tessera:emoji` after closing). They no longer take clicks with 3324b35 (a right-click on the desktop after closing them opens the desktop menu), so this is only bookkeeping, but worth a look.
10. **Data differences, not drawing**: Bluetooth on/off, the battery glyph, the brightness level and the Batteries widget's devices come from tessera-ui's own readers in Rust and from the QML services in QML, and disagree in the sandbox.
11. **Small drawing differences**: the dock's apps button and trash glyphs, Foot's icon frame, the emoji category pill, the calendar widget's header, generic desktop file icons, the separators in the desktop menu, the dock's magnification while its menu is open.

Not seen: glass misaligned with content (every edge lines up at 2×), wrong blur or refraction placement, flicker within an animation (no frame moved away from the final state in any recording), z-order problems, text rendering differences beyond the tabular clock digits already noted in the menubar part.

## Notes for the next measurement

- main at `8ad126f` didn't build (three `m3-secondary` declarations in `ui/theme.slint`, fixed in `6cc00f2`); everything here is from `b8adf2d`.
- A virtual pointer that only clicks (a new `vptr` process) clicks at (−1, −1): move and click in one `vptr` call. Closing a menu with Esc while the pointer rests on the dock leaves both docks' hover tips up; glide the pointer away.
- Watch swap: with 8 agents building, the QML shell had 179 MB swapped out and its resident size halved. Record `VmSwap` with any memory number.
- `quiet.sh` can wait many minutes for a free moment: group tests into holds of about two minutes rather than one per test.

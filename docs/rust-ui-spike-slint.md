# Step 0 spike: the menu bar in Slint

Measured 4 November 2026 in sandbox arena 4. Code: `native/irice-ui-slint/` (the bar), `native/irice-ui-slint/qml-baseline/shell.qml` (the same bar in plain Quickshell, the fair QML number), `native/irice-ui-slint/tools/measure.py` (the measurement, identical for both). Plan: [rust-ui-plan.md](rust-ui-plan.md).

## In short

- **It works, both looks, at scale 1 and 2, with real Liquid Glass from the plugin.** But Slint has **no layer-shell support**: I wrote a small Slint *platform* of my own on smithay-client-toolkit (Wayland) and EGL (about 950 lines of Rust, accessibility and IME included). That glue is a one-time cost every later surface reuses.
- **Against the same bar in Quickshell:** about **40% of the memory** (61–72 MB vs 152–175 MB resident; 16–23 MB vs 47–70 MB of heap), **about 2× faster to first frame** (190–370 ms vs 410–560 ms to mapped), **about half the CPU per animation** (141–183 vs 279–363 ms per menu open and close, 20–26 vs 56–67 ms per workspace slide), the same 60 fps. Idle CPU is at the floor for both.
- **What we lose by leaving Slint's own backend:** its accessibility (AccessKit) and input-method code live in its winit backend only. I rebuilt both on Slint's *internal* API (`i-slint-core`, pinned to 1.18.1, not semver-stable). Both work end to end in the sandbox.
- **.slint reads close to QML** (bindings, `animate`, springs, `for`, `if`, states, callbacks), but there is no JavaScript: logic goes to Rust, and that's most of the cost of porting iRice's ~100k lines.

## What works

- **Layer-shell:** one top bar per output (Top layer, anchored top/left/right, exclusive zone 30), the menu as its own layer surface below it. Per-output: bars appear and go with outputs (`wl_output` names = Hyprland monitor names).
- **HiDPI:** `wp_fractional_scale_v1` + `wp_viewporter` (integer `preferred_buffer_scale` as a fallback). Tested at scale 1 and 2 on IRICE-TEST: crisp at 2, and the bar re-renders at the new scale when the output's scale changes live.
- **Left:** the drop logo (Phosphor-Fill), the active app's name (Hyprland's `activewindow` class → its desktop entry's `Name`), the workspace dots from socket2 events (`workspacev2`, `focusedmonv2`, `createworkspacev2`, …; one snapshot over the request socket per burst of events).
- **Right:** battery % and state (UPower's DisplayDevice over the system bus, `zbus`, PropertiesChanged), Wi-Fi on/off, SSID and strength (NetworkManager), the clock (re-armed on the minute). Material puts the clock first, as the shell does.
- **Fonts:** Inter (system, Glass), Google Sans Flex as "iRice Sans Flex" and Phosphor ×3 from the repo, memory-mapped and registered with Slint's font collection at run time (not compiled in).
- **The menu:** click the logo; 5 rows and a separator, the first a search field. Arrows (skipping the separator), Tab, Enter, Esc; hover highlights; click activates. Opens with a spring (`easing: spring(0.3)`: scale 0.92→1, 8 px drop, fade), closes with a quicker ease-in. Clicking another window closes it (it loses the keyboard), as on macOS. A11y and IME below.
- **Both looks from `$XDG_CONFIG_HOME/irice/config.json`** (`appearance.look`, `.mode`, `.materialSeed`, `.materialStyle`), **live**: the file is watched (inotify) and the bar restyles when the shell writes it. `IRICE_LOOK=glass|material` overrides for measuring.
  - Glass: a translucent strip; when the irice-glass plugin answers `ping`, the bar and the menu register their shapes exactly as GlassWindow does (`irice-glass set <monitor> irice:slintbar 1 <19 numbers>`, batched with `[[BATCH]]`, newest per surface wins, each request on its own thread so Hyprland never waits on the UI). The menu's shape follows its spring every frame; shapes are cleared on exit. Without the plugin it falls back to Hyprland's blur (60% tint, over `ignore_alpha`).
  - Material: tonal colours from the seed through the `material-colors` crate (spec 2025, as MaterialColor.js), pills behind the groups, the page-indicator spaces, a pill-shaped menu with state layers.
- **The workspace indicator slides** (a spring with a little bounce) between spaces; slots after the current one move aside.
- **Exit:** SIGTERM/SIGINT (signalfd in the loop, blocked before any thread starts) and stdin closing (watched only when stdin is a pipe) both end it cleanly in ~16 ms, glass shapes cleared. SIGUSR1 prints frame statistics.

Screenshots (sandbox IRICE-TEST only), menu open, "Files" typed, a row selected:

| | scale 1 | scale 2 |
|---|---|---|
| Glass, real plugin glass (Hyprland 0.56.2 arena) | `final-slint-glass-plugin-x1.png` | `final-slint-glass-plugin-x2.png` |
| Glass, Hyprland blur fallback (0.57.1 arena) | `final-slint-glass-x1.png` | `final-slint-glass-x2.png` |
| Material | `final-slint-material-x1.png` | `final-slint-material-x2.png` |
| The QML baseline, same | `final-qml-{glass,material}-x{1,2}.png` | |

(in the session scratchpad `shots/`; not committed.)

## Layer-shell: what it took

Slint 1.18.1 (current) has backends for winit, Qt, linuxkms and a testing one. **winit has no layer-shell**, so the winit backend (Slint's default on Linux) can't make a panel. The options I looked at:

| Option | Verdict |
|---|---|
| Slint's winit backend | No layer-shell in winit; `unstable-winit-030` only lets you tweak xdg-toplevel attributes. |
| Slint's Qt backend + LayerShellQt | Brings Qt back: the thing we're leaving. |
| `layer-shika` 0.3 (crates.io) | Slint + SCTK + FemtoVG, popups, fractional scale. **AGPL-3.0**, its own framework on top. Not used. |
| `spell-framework` 1.0 (GPL-3.0) | Slint + SCTK + Skia; uses Slint internals (`i-slint-core`, `i-slint-renderer-skia`) at 1.17. Not used. |
| **Our own Slint platform** (chosen) | `slint::platform::Platform` + one `WindowAdapter` per layer surface. |

What our own platform is (`src/wayland.rs`, `src/egl.rs`, `src/a11y.rs`):

1. **Wayland via smithay-client-toolkit 0.20:** `wlr-layer-shell` surfaces, seat (pointer, keyboard with xkb and client-side repeat), `wp_cursor_shape`, `wp_fractional_scale_v1` + `wp_viewporter`, `zwp_text_input_v3`, all in a calloop event loop together with signals, stdin, Slint's timers and a channel from the data threads.
2. **Drawing:** Slint's public `FemtoVGRenderer` (OpenGL ES) on a `wl_egl_window` per surface, one EGL context for the process (`khronos-egl`, libEGL loaded at run time). Traps: the context must be current *before* `FemtoVGRenderer::new` (it loads GL there); swap interval 0, pacing by `wl_surface.frame`.
3. **Glue to Slint:** a component is created after its surface is prepared (the platform hands the prepared adapter to `create_window_adapter`); configure → `ScaleFactorChanged` + `Resized`; pointer/keys → `WindowEvent`s (xkb keysyms mapped to Slint's `Key`s).
4. **Frame pacing:** draw only when Slint asked for a redraw *and* the previous frame was shown. Don't copy the "keep redrawing while `has_active_animations()`" pattern: Slint's animation clock is global, so every window redrew while any one animated (the bar re-rendered for the menu's spring: 709 bar frames in the first run, half of it waste). An animated property marks only its own window dirty; the loop just keeps ticking while something animates and nothing is pending.
5. **Not done:** xdg-popup / `create_child_window_adapter` (Slint's `PopupWindow` would draw *inside* the 30 px bar, so the menu is a separate component on its own surface), touch, drag and drop, clipboard, cursor shapes beyond the default, output hot-plug beyond add/remove.

## The numbers

Same arena, same settings: Glass look, dark, IRICE-TEST 1920×1080 at scale 1, the full iRice QML shell running beside it (its own menu bar moved off IRICE-TEST), one bar measured at a time, release build of the Slint bar. Each column is one run of `tools/measure.py`; the host's power profile was `power-saver` throughout (it skews every CPU number the same way). Runs 2–3 were on the container's Hyprland 0.57.1 (no glass plugin: Hyprland blur); run 4 on 0.56.2 with the plugin drawing the Slint bar's glass (the QML baseline doesn't register glass, so it gets Hyprland's blur).

| | Slint run 2 | Slint run 3 | Slint run 4 | QML baseline run 1 | QML run 3 | QML run 4 |
|---|---:|---:|---:|---:|---:|---:|
| Start-up: exec → layer mapped | 226 ms | 374 ms | 188 ms | 556 ms | 430 ms | 409 ms |
| Start-up: exec → own first-frame log | 224 ms | 369 ms | 241 ms | 643 ms | 546 ms | 575 ms |
| Resident (VmRSS) after 60 s | 71.6 MB | 70.8 MB | 61.5 MB | 175.3 MB | 162.4 MB | 151.9 MB |
| Private (clean+dirty) | 36.8 MB | 41.2 MB | 24.9 MB | 110.1 MB | 84.4 MB | 46.9 MB |
| PSS | 45.2 MB | 48.9 MB | 29.0 MB | 119.6 MB | 100.1 MB | 62.7 MB |
| Anonymous (heap) | 22.8 MB | 22.8 MB | 15.8 MB | 70.0 MB | 56.5 MB | 46.6 MB |
| Threads | 15 | 15 | 15 | 18 | 18 | 18 |
| Idle CPU over 60 s | 0.02 % | 0.08 % | 0.08 % | 0.05 % | 0.05 % | 0.08 % |
| CPU per menu open + close | 155 ms | 141 ms | 183 ms | 363 ms | 279 ms | 361 ms |
| CPU per workspace switch (slide) | 24 ms | 20 ms | 26 ms | 64 ms | 56 ms | 67 ms |
| Frame interval while animating, p50 / p95 | 17 / 17 ms | 17 / 17 ms | 17 / 17 ms | 16 / 18 ms | 16 / 31 ms | 16 / 17 ms |
| Exit | 15 ms, code 0 (stdin closed) | 16 ms | 214 ms (clears its glass first) | SIGTERM | SIGTERM | SIGTERM |

Private and PSS swing between runs for both (they count libraries shared with the other processes: the QML baseline shares Qt with the iRice shell); resident and anonymous memory are the steadier numbers.

(Run 1 of the Slint bar had the global-redraw mistake above: 164 ms per menu cycle, otherwise the same. Idle CPU is 1–8 clock ticks of 10 ms in 60 s for both: at the floor of what /proc can see.)

Frame detail, Slint (run 3): drawing a bar frame took 0.98 ms mean (p95 1.4 ms) on the CPU side, a menu frame 1.1 ms (p95 1.7 ms); the first frame after the menu maps is the slow one (26–47 ms: glyph atlas and shaders). Compositor-side pacing on IRICE-TEST from the plugin (`irice-glass frametimes`, run 4, frames on time / one vsync missed / two / longer): menu cycles, Slint 729 / 4 / 10 / 7 (worst 222 ms), QML 928 / 2 / 10 / 10 (worst 198 ms), the misses at the moments a menu surface maps or unmaps in both; workspace slides, Slint 257 / 0 / 0 / 0 (worst 16 ms), QML 304 / 2 / 0 / 0 (worst 33 ms). Intervals are between the compositor's `wl_surface.frame` callbacks (Slint) and `FrameAnimation.frameTime` (Qt): the closest the two expose; gaps over 250 ms (between animations) are left out.

Build and size:

| | Slint bar |
|---|---:|
| Release binary, stripped (opt-level 3, fat LTO, 1 codegen unit) | 11.2 MB (`.text` 6.6 MB, `.rodata` 3.7 MB: Unicode/ICU segmentation data, fonts' shaping tables, colour tables) |
| Crates (normal dependencies for this target, `cargo tree -e normal`) | 294 (304 with build deps; Cargo.lock lists 535 for every platform) |
| First release build | ~8 min (fat LTO); incremental ~1.5 min |
| Shared libraries linked | libwayland-client/-egl, libxkbcommon, libfontconfig (+freetype); libEGL/GLES dlopened |

Lines (non-blank, non-comment):

| | Slint | QML baseline |
|---|---:|---:|
| UI markup (`ui/bar.slint` / `shell.qml`) | 419 | 408 (all of it: Quickshell gives Hyprland, UPower, Networking, the clock) |
| Bar logic in Rust (`app.rs`, `main.rs`, `theme.rs`, `stats.rs`) | 836 | — |
| Data services (`hypr.rs` Hyprland + glass, `status.rs` UPower + NM) | 238 | — |
| Platform glue, once for all surfaces (`wayland.rs`, `egl.rs`, `a11y.rs`) | 945 | — |

The full iRice shell for reference: **469 MB resident on the real session** (Material, today); 244 MB in this arena (Glass, 68 threads, 146 MB anonymous).

## Accessibility

AccessKit's Unix adapter (`accesskit_unix` 0.24) on the AT-SPI bus, one adapter per surface; the tree is built from Slint's item tree (`src/a11y.rs`, a small port of the winit backend's `accesskit.rs`), rebuilt when a property it read changes, and only while a screen reader listens. Checked in the sandbox with `busctl` on the a11y bus (`org.a11y.Status.IsEnabled = true`, then walking `org.a11y.atspi.Accessible`):

```
application: 'irice-ui-slint'
  frame: 'Menu bar'
    push button: 'System menu' actions=click
    push button: 'Active app: Desktop' actions=click
    page tab list: 'Spaces'
      page tab: 'Space 1, empty' [selected] actions=click
      page tab: 'Space 2, empty' actions=click   (…to 5)
    push button: 'Battery 42 percent, charging' actions=click
    push button: 'Wi-Fi <ssid>, good signal' actions=click
    push button: 'Wednesday 4 November 2026, 13:50' actions=click
  frame: 'System menu'
    list box: 'System menu'
      entry: 'Search' [editable, focused]
      list item: 'About this computer' actions=click
      list item: 'System settings…' actions=click
      list item: 'Lock screen' actions=click
      list item: 'Sleep' actions=click
```

`DoAction(0)` on "System menu" over AT-SPI opened the menu. The QML baseline's tree (Qt's own AT-SPI) is the same shape, with Qt's quirks (hidden duplicates, the separator as an empty list item). What a screen reader would miss in ours:
- The typed text of the search field: the winit backend emits AccessKit text runs for TextInputs; my port doesn't (a few dozen more lines).
- Menu roles: Slint has no `menu`, `menu-item` or `menu-bar` accessible roles (AccessKit has them), so the menu reads as a list.
- AccessKit's application object has no `GetRoleName` (tools that call it on the root fail; Orca uses `GetRole`).
- Gotcha: a component that `inherits Text` keeps the implicit text role even with `accessible-role: none` (the icon glyphs were read out as private-use characters); wrapping the Text fixed it.

## Text input and input methods

`zwp_text_input_v3` is implemented in the platform: enabled while a Slint TextInput asks for an input method (Slint's internal `input_method_request`), with the cursor rectangle and password purpose; preedit and commit go to Slint as composition events. Tested end to end with a stand-in input method (`src/bin/fake-ime.rs`, `zwp_input_method_v2`; the sandbox has no fcitx5/IBus): it activates on the search field, shows the preedit "cafe", commits "café ✓", in both the Slint bar and the QML baseline. Difference: Qt hides the placeholder during preedit and underlines it; Slint draws the preedit highlighted and doesn't expose "has preedit" to .slint, so the "Search…" placeholder stays under it. Typing non-ASCII through xkb (wtype "héllo wörld") works. Untested: a real IME's candidate popup, right-to-left text, emoji (Slint shapes with parley/harfrust and falls back through fontconfig, so they should work; not verified here).

## Material colours: `material-colors` vs MaterialColor.js

`material-colors` 0.5 (MIT/Apache, a full port with spec 2021/2025/2026) against the shell's MaterialColor.js (MCU 0.4.0, spec 2025, phone), all 53 roles, 5 seeds × dark/light × tonalSpot/expressive/vibrant: **identical in 27 of 30 combinations**. The exception is **Expressive, dark**: `primary`, `surfaceTint`, `primaryContainer`, `onPrimaryContainer` differ (container and on-container are swapped in lightness, e.g. seed #3b6ea5 primaryContainer #a1c9fd in JS, #1b4975 in Rust). One of the two follows a different revision of the 2025 spec; before using it, pin which is right (or keep generating from MCU's TypeScript). Rust has 6 roles JS doesn't (palette key colours).

## What was hard

- **The platform.** 950 lines before the first pixel: EGL (context current before FemtoVG exists), SCTK's seat (a seat present at start never gets `new_seat`, so text-input had to be created on the keyboard capability), xkb → Slint keys, fractional scale, frame pacing, the global animation clock, input via a Hyprland quirk (a virtual pointer enters at (-1, -1) until it moves).
- **Accessibility and IME ride on internal API.** `i-slint-core`'s `WindowAdapterInternal`, `InternalKeyEvent`, `accessible_descendents`, `ItemRc`… are `#[doc(hidden)]`, not semver; pinned with `=1.18.1`. Every Slint upgrade can break them.
- **.slint's stricter language:** `let` is immutable, no loops in functions, `accessible-role` must be constant (so separator rows and items are different elements), globals are per window (the `Theme` global is filled once per bar and once for the menu; a QML singleton is shared).
- **Build:** 8 minutes for the first release build, 11 MB binary.
- **Measuring in a nested sandbox:** the glass plugin only loads where Hyprland matches the build it was made for (0.56.2; arena runs before the container rollback were on 0.57.1).

## What Slint can't do (yet)

- **Layer-shell, popups as real surfaces, accessibility and IME outside its winit backend:** all on us (done here, but on internal API for the last two).
- **No custom shaders** (no `ShaderEffect`): iRice's QML shaders (the glass fallback `glass.frag`, squircles, `materialshape.frag`, edge fades) would need Slint built-ins, images, or the compositor. The plugin glass works fine, since it's drawn by Hyprland.
- **No backdrop blur** of its own (fine: the compositor blurs).
- **Partial redraw** only in the software renderer; FemtoVG redraws the whole surface each frame (cheap at 1920×30; matters for big panels).
- **No accessible menu roles; no "has preedit"** property on TextInput.
- Popups inside a window only (`PopupWindow`), unless the platform implements child windows.

## Licensing

Slint 1.18.1 is `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0`. iRice (GPL-3.0-or-later) can use it under GPL-3.0: the combined program is then GPL-3.0 (a future "or later" move would need Slint's permission or the royalty-free licence, which allows desktop apps with attribution but not embedded devices). Everything else here is MIT/Apache-2.0 (SCTK, wayland-rs, zbus, AccessKit, material-colors, khronos-egl) or MIT/Unicode (ICU data). `layer-shika` (AGPL-3.0) was deliberately not used.

## How close .slint reads to QML

Close for the visual side. The same capsule:

```qml
// QML (qml-baseline/shell.qml)
Rectangle {
    id: capsule
    x: 4 + Math.max(0, spaces.current) * spaces.slot
    width: spaces.cap; height: shell.material ? 8 : 18
    radius: height / 2
    color: shell.dotActive
    Behavior on x { SpringAnimation { spring: 4; damping: 0.32 } }
}
```
```slint
// Slint (ui/bar.slint)
capsule := Rectangle {
    x: 4px + max(0, current) * slot;
    width: cap; height: Theme.material ? 8px : 18px;
    border-radius: self.height / 2;
    background: Theme.dot-active;
    animate x { duration: 380ms; easing: spring(0.25); }
}
```

What maps one to one: property bindings, `id`s (`name := Element`), `animate` = `Behavior`, `spring()` (velocity carries over on retarget, like SpringAnimation), `for x[i] in model:` = Repeater, `if cond: Element` = Loader/visible, `states` and transitions, `callback` = signal, `TouchArea` = MouseArea, `FocusScope` + `key-pressed` = Keys, layouts (`HorizontalLayout` = Row/RowLayout). What doesn't: no anchors (x/y expressions or layouts; and a layout not given a width fills its parent, a trap), units on lengths (`px`), no JavaScript (only expressions and small functions; arrays, string building, `Date`, JSON parsing, regexes all go to Rust), models are Rust `VecModel`s, no `Connections`, no singletons shared across windows (globals per component), no `Process`, `FileView`, `IpcHandler`: Quickshell's services become Rust code.

Porting estimate from this bar: the QML markup translates nearly line for line (408 → 419 lines). The Rust side (1,074 lines here, without the platform) is where iRice's JavaScript and Quickshell services go: of the ~100k lines, the share that's JS logic and service code will cost the most, as planned in "Services first where shared".

## Reproduce

```
distrobox enter irice -- bash -c 'cd native/irice-ui-slint && cargo build --release'
IRICE_ARENA=4 ./dev.sh up
IRICE_ARENA=4 ./dev.sh run python3 native/irice-ui-slint/tools/measure.py slint   # or qml
IRICE_ARENA=4 ./dev.sh run native/irice-ui-slint/target/release/irice-ui-slint --material-roles '#3b6ea5' dark tonalSpot
IRICE_ARENA=4 ./dev.sh down
```
`IRICE_DEBUG=1` logs configures, clicks, menu and text-input events and every glass request; `IRICE_GLASS_FORCE=1` sends glass shapes without the plugin; `IRICE_NO_A11Y=1` skips AccessKit; `kill -USR1` prints frame statistics.

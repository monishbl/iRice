# The Material look (Material 3 Expressive)

Settings → Appearance → Look: **Glass** (macOS Tahoe, the original) or
**Material** (Material 3 Expressive, as in Android 16). Every feature is the
same in both; only how it looks and moves changes. Decided with the user
(2 Nov 2026):

- **Layout:** Android panels in the same places. The top bar and dock stay;
  Control Center becomes Android's Quick Settings (pill tiles, big sliders),
  Notification Center its notification shade, menus and dialogs Material's.
- **Colour:** Material You, from the wallpaper (or a chosen seed colour), in a
  chosen style (Tonal, Vibrant, Expressive, Neutral).
- **Surfaces:** opaque tonal surfaces (no glass, no blur): faithful, and the
  lightest option on an integrated GPU.
- **Reach:** the whole system: GTK/Qt apps, window corners and controls,
  cursor, lock and login screens follow the look.
- **Names:** "Google" is a trademark (and Google Sans Flex's licence says so):
  the looks are called Glass and Material in the UI.

Second round of decisions (same day):

- **Settings** in Material: Android two-pane (a list with tonal circle icons
  and a pill on the current page; a headline, a search pill, Expressive
  segmented lists).
- **Switching** looks shows a screen: the iRice mark morphs from a glass
  squircle through Material's shapes into a cookie (or back) over the
  blurred wallpaper while the shell restyles out of sight.
- **Pickers:** live preview cards (`common/LookPreview.qml`) at the top of
  Appearance, and a first-login welcome (`modules/lookswitch/Welcome.qml`,
  once, on fresh installs).
- **Different, not restyled, in Material:** Quick Settings, the notification
  shade, the lock screen (Android 16), search (Google style) and the app
  drawer, the taskbar, plus its own sound set and an original default
  wallpaper.

## How it's built (v2: two separate UI trees, one core)

The first build mixed Material into Glass's own files (about 240 of them)
and switched live; it broke Control Center on real hardware and was reverted
(2 Nov). This one keeps them apart:

- **`shell/`** is Glass, as before. Its UI files are untouched; the only
  Glass changes are plumbing: Config keys (`appearance.look`, `materialSeed`,
  `materialStyle`, `materialSounds`, `materialCursor`), an "experimental"
  Look row in Settings → Appearance, and the scripts below.
- **`material/`** holds only the Material look's own files, in the same
  layout as `shell/`: its Theme and controls (`common/`), surfaces
  (`glass/`), the panels it redesigns or restyles (`modules/`), its own
  versions of a few services (sounds, app colours, pointer, window corners),
  fonts, sounds, wallpapers. Many are copies of Glass files with Material
  branches (`Theme.material` is always true there).
- **The Material root** is assembled at every start by
  `shell/assets/scripts/material-root.py` into
  `$XDG_CACHE_HOME/irice/material-shell`: a link for every file of
  `shell/`, except where `material/` has its own. Quickshell resolves
  `qs.common` inside the root it runs, so a Glass file used unchanged picks
  up the Material Theme. Services and settings data are the same files.
- **Starting:** `launch-shell.sh` reads `appearance.look` and starts the
  Glass folder or the Material root (whatever the arguments named), marks the
  shell `IRICE_SHELL=1`, and for Material starts a guard: no lock screen
  answering within 20 s (or the shell quits first) → back to Glass, with a
  notification. A broken Material can't leave you without a shell.
- **Switching** restarts the shell: `switch-look.sh` shows a separate
  full-screen cover (`assets/lookswitch/`, like the update screen), restarts
  through `repair-shell.sh` (in the dev sandbox, its own reload), waits for
  the new shell to answer, then fades the cover. Leaving Material reloads
  Hyprland's config (window styling it set live).
- **Talking to the shell:** `shell-ipc.sh` finds whichever root runs (idle
  lock, file sharing, `./dev.sh ipc`). `start-idle.sh` replaces a listener
  from before it (its lock command named one folder).
- **Not in v2 yet:** the login screen stays Glass; the irice-glass plugin's
  Material window controls and the login-screen sync are left out.
- **Lesson kept:** Material's pull is 0, and Glass's ElasticPull divides by
  it (0/0 → NaN → a pressed panel never drew again). Material has its own
  no-op ElasticPull/ElasticControl. Test with real clicks, not only IPC.

## Spec numbers (from Google's Compose source and material-color-utilities)

**Shape scale (px):** none 0 · extraSmall 4 · small 8 · medium 12 · large 16 ·
largeIncreased 20 · extraLarge 28 · extraLargeIncreased 32 ·
extraExtraLarge 48 · full 50%.

**Shapes** (`MaterialShapes.kt`, unit square): Circle, Square (rounding .30),
Slanted, Arch, Fan, Arrow, SemiCircle, Oval, Pill, Triangle, Diamond,
ClamShell, Pentagon, Gem, Sunny (star 8, inner .8, round .15), VerySunny,
Cookie4/6/7/9/12 (7: star 7 inner .75 round .5; 9, 12: inner .8 round .5),
Ghostish, Clover4Leaf, Clover8Leaf, Burst, SoftBurst, Boom, SoftBoom, Flower,
Puffy, PuffyDiamond, PixelCircle, PixelTriangle, Bun, Heart.

**Morphs:** button press: round → corner XS/S 8, M 12, L/XL 16 (effects
spring, no bounce). Toggle selected: round → square XS/S 12, M 16, L/XL 28.
Loading indicator: SoftBurst → Cookie9 → Pentagon → Pill → Sunny → Cookie4 →
Oval, a morph every 650 ms (spring ζ .6, k 200), +90° each, plus 360° per
4666 ms.

**Springs (ζ / k, mass 1):** expressive spatial fast .6/800 (9% overshoot),
default .8/380, slow .8/200; effects (never bounce) fast 1/3800, default
1/1600, slow 1/800. Standard scheme spatial .9/1400, .9/700, .9/300.
Spatial = position, size, shape, rotation; effects = colour, opacity.

**Easing (legacy):** emphasized decelerate (.05,.7,.1,1), emphasized
accelerate (.3,0,.8,.15), standard (.2,0,0,1). Durations 50–1000 ms
(short1 50 … medium2 300 … long2 500 … extraLong4 1000).

**Type** (size/line, tracking, weight; emphasized = one step heavier):
display 57/64, 45/52, 36/44 · headline 32/40, 28/36, 24/32 · title 22/28 w400,
16/24 .15 w500, 14/20 .1 w500 · body 16/24 .5, 14/20 .25, 12/16 .4 ·
label 14/20 .1 w500, 12/16 .5 w500, 11/16 .5 w500.

**State layers:** hover 8%, focus 10%, pressed 10%, dragged 16% of the content
colour. Disabled: content 38%, container 10–12% of onSurface.

**Components:**
- Buttons XS 32 · S 40 · M 56 · L 96 · XL 136 high; round by default.
- Icon buttons the same sizes; narrow/default/wide padding.
- Button group: standard 12 gap, pressed grows 15% and squeezes neighbours;
  connected 2 gap, inner corners 8 (pressed 4, selected round).
- Split button: 2 gap, inner 4.
- FAB 56/80/96 (corner 16/20/28).
- Switch: track 52×32, 2 outline; handle 16 off, 24 on or with icon, 28
  pressed.
- Slider: handle 4×44 (2 pressed) with a 6 gap each side; track
  XS 16 · S 24 · M 40 · L 56 · XL 96 (corners 8/8/12/16/28); inactive
  secondaryContainer; stop dots 4.
- Loading indicator 48 (shape 38).
- Wavy progress: linear track 4, amplitude 3, wavelength 40; circular 40/48,
  stroke 4.
- Lists: segmented items 2 apart, group corner 16, inner 4; hover 12,
  pressed/selected 16.
- Menus: groups in surfaceContainerLow, 2 apart, items 44, selected
  tertiaryContainer.
- Dialog: surfaceContainerHigh, corner 28.
- Snackbar: inverseSurface, corner 4.
- Chips: 32 high, corner 12 → pill when selected.
- Toolbars: 64 high, floating pill; vibrant = primaryContainer.
- Nav indicator: 56×32 pill, secondaryContainer.

**Expressive, as rules:** contrast of shapes (pills next to squarer corners,
shapes change with state); size hierarchy (the key action much bigger);
bolder containers (primary/tertiary containers, vibrant toolbars, segmented
groups 2 apart); emphasized type for headlines and selections; spatial
springs that overshoot for movement, effects springs for colour. Keep
familiar patterns: labels and lists stay.

Sources: github.com/androidx/androidx compose/material3 (tokens,
MaterialShapes.kt, MotionScheme.kt), graphics/graphics-shapes,
material-foundation/material-color-utilities, material-components-android
slider tokens, design.google/library/expressive-material-design-google-research.

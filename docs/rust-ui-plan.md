# The interface in Rust: plan

Decided 4 November 2026: Tessera's interface moves from Quickshell/QML to Rust, all of it, both looks. This file is the plan; it changes as we learn.

## Why, and what we expect

- Today (real session, Material): the shell is one Quickshell process, 469 MB RSS, 0.3 % CPU at idle; 458 QML files, ~100k lines of QML and JS; 109 services.
- Hoped for: much less memory and a faster start, no JavaScript engine or QML runtime. Not expected: big CPU gains at idle (already near zero) or a lighter GPU (both draw on the GPU). Every step is measured against docs/benchmarks-baseline.md, docs/benchmarks-events.md and the QML version of the same piece.

## How we move without breaking your desktop

- **Piece by piece, side by side.** A Rust shell process (`tessera-ui`) runs next to the QML shell. Each piece (a "surface": menu bar, dock, notifications, OSD, Control Center, Search, App Drawer, lock, Settings, …) moves when its Rust version matches in both looks; the QML shell then stops drawing it (one switch per surface). Until then the QML one stays.
- **Same settings, same commands.** `tessera-ui` reads and writes the same config.json and state files, answers the same IPC commands (shell-ipc.sh, the smoke check), and uses the same glass plugin protocol, so nothing about your setup changes as pieces move.
- **Services first where shared.** System services (battery, network, sound, Bluetooth, notifications server, tray, MPRIS, Hyprland state) are written once in Rust and serve every Rust surface; irice-events is the start of that layer.
- **The lock screen moves last** (it's the security boundary), with its own review.
- Each moved surface: tests, the smoke check in both looks, sandbox screenshots shown to you before it reaches your laptop.

## Step 0: pick the toolkit by measurement (done: Slint)

The same menu bar built twice, each in a worktree, and compared on:

- Wayland layer-shell (top bar, exclusive zone, per monitor, HiDPI), and Hyprland's glass plugin regions.
- Both looks: Glass (translucent, blur from the plugin) and Material 3 (tonal colours, springs, shapes).
- Text: Inter / Google Sans Flex, emoji, right-to-left; an input field with an input method.
- Accessibility (an AT-SPI tree a screen reader can read), keyboard focus.
- Animation smoothness (frame times), start-up to first frame, idle CPU, memory.
- How much code the bar takes, and how close it reads to the QML (porting cost for the other ~100k lines).

Candidates:

- **Slint**: declarative `.slint` markup close to QML (bindings, states, animations), GPU renderers (Skia / FemtoVG), accessibility through AccessKit, GPLv3-compatible. Open question: layer-shell support.
- **iced** (System76's fork used by COSMIC, or iced with a layer-shell crate): a Rust desktop's panels and applets already run on it with layer-shell; accessibility and input methods through the COSMIC work. Open question: how much of COSMIC's styling comes along, and Material/Glass freedom.

GTK4 (gtk4-rs + gtk4-layer-shell) is the fallback if both fail on layer-shell or accessibility; it keeps a large C stack, which is what we're trying to leave.

## After step 0

1. `tessera-ui` skeleton: the chosen toolkit, theme tokens for both looks (from Theme.qml / Material colours), config and state readers, the IPC server, the per-surface switch in the QML shell.
2. Surfaces, roughly cheapest and most-seen first: menu bar, dock, OSD, notifications and banners, Control Center, Search, App Drawer, Today, sidebar (AI, translator), switcher, overview, screen tools, Settings (largest), lock and greeter (last).
3. When every surface has moved: Quickshell leaves the dependencies; the QML stays in git history.

## Where it stands

- **Step 0, done (4 November 2026): Slint.** Measured in docs/rust-ui-spike-slint.md: about 40% of QML's memory for the same bar, half the CPU per animation, a working screen-reader tree and input methods; layer-shell through a platform of our own. iced (System76's fork) had layer-shell built in, but its panel had no accessibility tree and its input methods were unproven. GTK4 wasn't needed.
- **Step 1, the skeleton (native/tessera-ui):**
  - the spike's Slint platform (Wayland layer-shell, EGL, accessibility, input methods), its menu bar and its data threads (Hyprland, UPower, NetworkManager, the glass plugin under either name);
  - the look from config.json, with Material's colours from the seed (checked against the shell's MaterialColor.js, every role, both modes) and the wallpaper's seed computed in-process with matugen's steps (checked against matugen);
  - the per-surface switch: Settings' `rust.surfaces` (empty by default), services/RustUi.qml starts `tessera-ui --surfaces …`, and a QML surface steps aside only while tessera-ui says it draws it (`RustUi.drawing(name)`); if tessera-ui ends, the QML one is back;
  - tests: the crate's (`cargo test`) and tests/test_rust_ui.py (the QML and Rust sides agree on every surface; the default moves nothing; the binary's answers).
- **Panels (4 November 2026):** the emoji picker and the clipboard history in Rust (rust.surfaces: emoji, clipboard; previews), opened as before through Panels: RustUi.qml forwards each open and close to tessera-ui on its stdin, tessera-ui says when one closed itself and asks the shell for what only it does (docs/rust-ui-surfaces.md, "Panels").
- **The dock (preview, `rust.surfaces: dock`):** both looks' docks (pinned, running, recent, minimized, Downloads, Trash; magnification, hover lift, labels and tooltips, auto-hide, the right-click menu), with services later surfaces reuse: Hyprland's windows (services/clients.rs), desktop entries and launching (services/apps.rs), the icon theme (services/icons.rs) and the shell's icon styles drawn on the CPU (src/appicon.rs). Not yet: the Downloads stack (it opens the folder), unread badges, minimized windows' thumbnails, App Exposé from the dock.
- **The menu bar, nearly at parity (4 November 2026):** every item of MenuBar.qml in both looks, its own menus (system, app, Window, tray over dbusmenu, a space's), the spaces as Workspaces.qml lists them, text that flips over light wallpapers, auto-hide and fullscreen, and the island (idle, live activities, alerts, cards, the volume row, the pin, the pill over fullscreen apps). New services: MPRIS, the tray (StatusNotifier host, and watcher when none runs), BlueZ, the wallpaper's top. What the QML shell still owns reaches it over a bridge (services/bridge.rs: RustUi.qml writes state and the island's alerts to its stdin; it asks back with `shell-ipc.sh call rustui act …`, modules/menubar/RustBridge.qml); the status items open the QML popovers under them until the Control Center moves. Gaps are listed at the top of src/surfaces/menubar/mod.rs.
- **The wallpaper (background) and its picker (wallpaper), previews:** the picture per screen at its pixels with the QML's crossfade (a screen plugged in later fades in quickly) and the lock's settle, every dynamic mode (time of day, light and dark pair) and a live wallpaper's poster (the video itself stays the QML's); the QML window keeps the desktop's widgets on the bottom layer. The picker is a panel opened through the shell; choosing and Open folder are the shell's (`wallpaper.set`, `wallpaper.openFolder`). Background.qml tells tessera-ui the day's sunrise and sunset and LockFx's steps (`RustUi.tell`).
- **The desktop (preview, `rust.surfaces: desktop`):** the widgets on their grid (all sixteen kinds, drag to snap, sizes, remove, Edit Widgets' grid and badges, Smart Stacks), the files on the desktop (stacks, drag to place, the file menu), and the right-click menus, both looks, on the bottom layer of the first screen. What the QML shell still owns (weather, events, to-dos, Screen Time, the player, Bluetooth batteries, the notes' text) reaches it from modules/desktop/DesktopFeed.qml. Not yet: the gallery and Widget Options (still QML panels), the large calendar's Week and To-do views, filled Material Symbols, LockFx's zoom, the GPU graph.
- **Next:** the menu bar's last gaps, then the order in "After step 0". The IPC server waits for the first surface that needs a command of its own; until then the QML shell answers them.


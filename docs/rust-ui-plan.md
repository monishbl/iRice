# The interface in Rust: plan

Decided 4 November 2026: iRice's interface moves from Quickshell/QML to Rust, all of it, both looks. This file is the plan; it changes as we learn.

## Why, and what we expect

- Today (real session, Material): the shell is one Quickshell process, 469 MB RSS, 0.3 % CPU at idle; 458 QML files, ~100k lines of QML and JS; 109 services.
- Hoped for: much less memory and a faster start, no JavaScript engine or QML runtime. Not expected: big CPU gains at idle (already near zero) or a lighter GPU (both draw on the GPU). Every step is measured against docs/benchmarks-baseline.md, docs/benchmarks-events.md and the QML version of the same piece.

## How we move without breaking your desktop

- **Piece by piece, side by side.** A Rust shell process (`irice-ui`) runs next to the QML shell. Each piece (a "surface": menu bar, dock, notifications, OSD, Control Center, Search, App Drawer, lock, Settings, …) moves when its Rust version matches in both looks; the QML shell then stops drawing it (one switch per surface). Until then the QML one stays.
- **Same settings, same commands.** `irice-ui` reads and writes the same config.json and state files, answers the same IPC commands (shell-ipc.sh, the smoke check), and uses the same glass plugin protocol, so nothing about your setup changes as pieces move.
- **Services first where shared.** System services (battery, network, sound, Bluetooth, notifications server, tray, MPRIS, Hyprland state) are written once in Rust and serve every Rust surface; irice-events is the start of that layer.
- **The lock screen moves last** (it's the security boundary), with its own review.
- Each moved surface: tests, the smoke check in both looks, sandbox screenshots shown to you before it reaches your laptop.

## Step 0: pick the toolkit by measurement (now)

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

1. `irice-ui` skeleton: the chosen toolkit, theme tokens for both looks (from Theme.qml / Material colours), config and state readers, the IPC server, the per-surface switch in the QML shell.
2. Surfaces, roughly cheapest and most-seen first: menu bar, dock, OSD, notifications and banners, Control Center, Search, App Drawer, Today, sidebar (AI, translator), switcher, overview, screen tools, Settings (largest), lock and greeter (last).
3. When every surface has moved: Quickshell leaves the dependencies; the QML stays in git history.
